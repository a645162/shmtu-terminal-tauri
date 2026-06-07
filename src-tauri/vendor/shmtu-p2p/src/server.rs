use std::collections::HashMap;
use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::client::read_loop_inner;
use crate::crypto::{self, SecureKey};
use crate::discovery::{generate_qr_payload, get_local_ips};
use crate::protocol::*;
use crate::transfer::{
    compute_checksum, take_pending_incoming_transfer, TransferEngine, TransferProgress,
};
use crate::{
    DataReceived, P2PError, P2PEvent, P2PInfo, P2PSession, PairingRequest, TransferComplete,
    TransferError,
};

/// 配对请求超时时间（秒）
const PENDING_SESSION_TIMEOUT_SECS: u64 = 60;

/// 配对失败尝试最大次数（超过后拒绝该 IP 的连接）
const MAX_PAIR_ATTEMPTS: u32 = 5;

/// 配对失败记录过期时间（秒）
const PAIR_ATTEMPT_EXPIRY_SECS: u64 = 600; // 10 分钟

/// 配对失败清理间隔（秒）
const PAIR_ATTEMPT_CLEANUP_INTERVAL_SECS: u64 = 300; // 5 分钟

/// 服务端待配对会话
struct PendingSession {
    read_half: Arc<Mutex<Option<tokio::io::ReadHalf<TcpStream>>>>,
    write_half: Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    peer_ip: String,
    peer_port: u16,
    device_name: String,
    /// 配对码（用于加密协商）
    pair_code: String,
    reconnect_ips: Vec<String>,
    reconnect_port: Option<u16>,
}

/// 服务端已配对会话
#[allow(dead_code)]
struct ActiveSession {
    write_half: Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    session_id: String,
    peer_ip: String,
    peer_port: u16,
    peer_device_name: String,
    is_incoming: bool,
    /// 加密密钥
    encryption_key: Option<SecureKey>,
    pair_code: String,
    reconnect_ips: Vec<String>,
    reconnect_port: Option<u16>,
}

#[derive(Clone)]
struct ActiveSessionHandle {
    session_id: String,
    peer_ip: String,
}

/// 配对失败记录
#[derive(Debug)]
struct PairAttempt {
    /// 失败次数
    count: u32,
    /// 首次失败时间
    #[allow(dead_code)]
    first_attempt_at: std::time::Instant,
    /// 最后一次失败时间
    last_attempt_at: std::time::Instant,
}

/// P2P 服务端
pub struct P2PServer {
    port: u16,
    pair_code: PairCode,
    local_ips: Vec<String>,
    qr_payload: String,
    listener: Arc<Mutex<Option<TcpListener>>>,
    shutdown_tx: Arc<Mutex<Option<mpsc::UnboundedSender<()>>>>,
    pending_sessions: Arc<RwLock<HashMap<String, PendingSession>>>,
    active_sessions: Arc<RwLock<HashMap<String, ActiveSession>>>,
    known_sessions: Arc<RwLock<Vec<P2PSession>>>,
    event_tx: mpsc::UnboundedSender<P2PEvent>,
    /// 配对失败尝试追踪: IP -> 失败记录
    pair_attempts: Arc<RwLock<HashMap<String, PairAttempt>>>,
}

impl P2PServer {
    /// 启动 P2P 服务端
    /// 如果指定端口被占用则自动递增重试（最多尝试 10 个端口）
    pub async fn start(
        event_tx: mpsc::UnboundedSender<P2PEvent>,
        preferred_port: u16,
        known_sessions: Arc<RwLock<Vec<P2PSession>>>,
    ) -> Result<Self, P2PError> {
        let pair_code = PairCode::generate();
        let local_ips = get_local_ips();

        // 绑定端口，如果被占用则自动 +1 重试
        let mut port = preferred_port;
        let listener = loop {
            match TcpListener::bind(("0.0.0.0", port)).await {
                Ok(l) => break l,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::AddrInUse && port < preferred_port + 10 {
                        tracing::warn!("[P2P] Port {} in use, trying {}", port, port + 1);
                        port += 1;
                        continue;
                    }
                    return Err(P2PError::Io(e));
                }
            }
        };
        let actual_port = listener.local_addr()?.port();

        let qr_payload = generate_qr_payload(actual_port, &pair_code);

        tracing::info!(
            "[P2P] Server started on port {}, pair_code={}",
            actual_port,
            pair_code
        );

        let pair_attempts: Arc<RwLock<HashMap<String, PairAttempt>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let server = Self {
            port: actual_port,
            pair_code,
            local_ips,
            qr_payload,
            listener: Arc::new(Mutex::new(Some(listener))),
            shutdown_tx: Arc::new(Mutex::new(None)),
            pending_sessions: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            known_sessions,
            event_tx,
            pair_attempts: pair_attempts.clone(),
        };

        // 启动接受连接循环
        server.start_accept_loop();

        // 启动配对失败记录清理任务
        server.start_pair_attempt_cleanup(pair_attempts);

        Ok(server)
    }

    /// 启动配对失败记录定时清理任务
    fn start_pair_attempt_cleanup(&self, pair_attempts: Arc<RwLock<HashMap<String, PairAttempt>>>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(
                PAIR_ATTEMPT_CLEANUP_INTERVAL_SECS,
            ));
            loop {
                interval.tick().await;
                let mut attempts = pair_attempts.write().await;
                let now = std::time::Instant::now();
                attempts.retain(|_ip, attempt| {
                    now.duration_since(attempt.last_attempt_at).as_secs() < PAIR_ATTEMPT_EXPIRY_SECS
                });
                if !attempts.is_empty() {
                    tracing::debug!(
                        "[P2P] Cleaned up pair attempt records, {} IPs still tracked",
                        attempts.len()
                    );
                }
            }
        });
    }

    /// 启动接受连接循环
    fn start_accept_loop(&self) {
        let listener = self.listener.clone();
        let pending_sessions = self.pending_sessions.clone();
        let active_sessions = self.active_sessions.clone();
        let known_sessions = self.known_sessions.clone();
        let event_tx = self.event_tx.clone();
        let expected_pair_code = self.pair_code.clone();
        let shutdown_tx = self.shutdown_tx.clone();
        let pair_attempts = self.pair_attempts.clone();

        tokio::spawn(async move {
            let (shutdown_sender, mut shutdown_rx) = mpsc::unbounded_channel::<()>();
            {
                let mut tx = shutdown_tx.lock().await;
                *tx = Some(shutdown_sender);
            }

            let listener = {
                let mut l = listener.lock().await;
                l.take()
            };

            let listener = match listener {
                Some(l) => l,
                None => {
                    tracing::error!("[P2P] No listener available");
                    return;
                }
            };

            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                tracing::info!("[P2P] Incoming connection from {}", addr);
                                let pair_code = expected_pair_code.clone();
                                let event_tx = event_tx.clone();
                                let pending = pending_sessions.clone();
                                let active = active_sessions.clone();
                                let known = known_sessions.clone();
                                let pair_attempts = pair_attempts.clone();

                                tokio::spawn(async move {
                                    if let Err(e) = handle_incoming_connection(
                                        stream,
                                        addr,
                                        &pair_code,
                                        event_tx,
                                        known,
                                        pending,
                                        active,
                                        pair_attempts,
                                    ).await {
                                        tracing::warn!("[P2P] Error handling connection from {}: {}", addr, e);
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::error!("[P2P] Accept error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("[P2P] Accept loop shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// 停止服务端
    pub async fn stop(&self) -> Result<(), P2PError> {
        // 发送关闭信号
        {
            let tx = self.shutdown_tx.lock().await;
            if let Some(tx) = tx.as_ref() {
                let _ = tx.send(());
            }
        }

        // 关闭 listener
        {
            let mut listener = self.listener.lock().await;
            *listener = None;
        }

        // 清空所有会话（SecureKey 的 Drop 会自动清零密钥）
        {
            let mut pending = self.pending_sessions.write().await;
            pending.clear();
        }
        {
            let mut active = self.active_sessions.write().await;
            for (_id, session) in active.drain() {
                let frame = encode_disconnect("Server shutting down")?;
                let mut wh = session.write_half.lock().await;
                let _ = wh.write_all(&frame).await;
                let _ = wh.flush().await;
            }
        }

        tracing::info!("[P2P] Server stopped");
        Ok(())
    }

    /// 获取服务端信息
    pub fn info(&self) -> P2PInfo {
        P2PInfo {
            port: self.port,
            pair_code: self.pair_code.as_str().to_string(),
            local_ips: self.local_ips.clone(),
            qr_payload: self.qr_payload.clone(),
            is_running: true,
        }
    }

    /// 获取 QR 载荷
    pub fn qr_payload(&self) -> String {
        self.qr_payload.clone()
    }

    /// 接受配对
    /// 返回 Ok(Some(key)) 如果加密协商成功
    pub async fn accept_pairing(&self, session_id: &str) -> Result<Option<SecureKey>, P2PError> {
        let (result, _, _) = self
            .accept_pairing_with_session(session_id, None)
            .await?;
        Ok(result)
    }

    pub async fn accept_pairing_with_session(
        &self,
        session_id: &str,
        disconnect_tx: Option<mpsc::UnboundedSender<String>>,
    ) -> Result<
        (
            Option<SecureKey>,
            crate::P2PSession,
            Option<Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>>,
        ),
        P2PError,
    > {
        let pending_session = {
            let mut pending = self.pending_sessions.write().await;
            pending.remove(session_id)
        };

        if let Some(pending) = pending_session {
            let new_session_id = uuid::Uuid::new_v4().to_string();
            accept_pending_session(
                pending,
                session_id,
                new_session_id,
                self.event_tx.clone(),
                self.active_sessions.clone(),
                disconnect_tx,
            )
            .await
        } else {
            Err(P2PError::SessionNotFound(session_id.to_string()))
        }
    }

    /// 拒绝配对
    pub async fn reject_pairing(&self, session_id: &str) -> Result<(), P2PError> {
        let pending_session = {
            let mut pending = self.pending_sessions.write().await;
            pending.remove(session_id)
        };

        if let Some(pending) = pending_session {
            let reject = PairReject {
                reason: "Rejected by user".to_string(),
            };
            let frame = encode_pair_reject(&reject)?;
            {
                let mut wh = pending.write_half.lock().await;
                let _ = wh.write_all(&frame).await;
                let _ = wh.flush().await;
            }
            tracing::info!("[P2P] Pairing rejected for session {}", session_id);
            Ok(())
        } else {
            Err(P2PError::SessionNotFound(session_id.to_string()))
        }
    }

    /// 发送数据到指定会话
    pub async fn send_data(&self, session_id: &str, data: &[u8]) -> Result<(), P2PError> {
        let session = {
            let active = self.active_sessions.read().await;
            active.get(session_id).map(|session| {
                (
                    session.write_half.clone(),
                    session.encryption_key.clone(),
                    session.peer_ip.clone(),
                    session.peer_port,
                    session.session_id.clone(),
                    session.pair_code.clone(),
                )
            })
        };

        if let Some((write_half, encryption_key, peer_ip, peer_port, active_session_id, pair_code)) = session {
            let transfer_id = uuid::Uuid::new_v4().to_string();
            let event_tx = self.event_tx.clone();
            let engine = TransferEngine::new(
                write_half,
                peer_ip,
                peer_port,
                active_session_id,
                pair_code,
                encryption_key,
            );
            engine.send(data, &transfer_id, event_tx, session_id).await
        } else {
            Err(P2PError::SessionNotFound(session_id.to_string()))
        }
    }

    /// 断开会话
    pub async fn disconnect(&self, session_id: &str) -> Result<(), P2PError> {
        let active_session = {
            let mut active = self.active_sessions.write().await;
            active.remove(session_id)
        };

        if let Some(session) = active_session {
            let frame = encode_disconnect("User disconnect")?;
            {
                let mut wh = session.write_half.lock().await;
                let _ = wh.write_all(&frame).await;
                let _ = wh.flush().await;
            }
            tracing::info!("[P2P] Disconnected session {}", session_id);
            Ok(())
        } else {
            // 也检查待配对会话
            let mut pending = self.pending_sessions.write().await;
            if pending.remove(session_id).is_some() {
                tracing::info!("[P2P] Removed pending session {}", session_id);
                Ok(())
            } else {
                Err(P2PError::SessionNotFound(session_id.to_string()))
            }
        }
    }

    /// 获取活跃会话的写半（供 P2PManager 注册）
    pub async fn get_active_write_half(
        &self,
        session_id: &str,
    ) -> Option<Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>> {
        let active = self.active_sessions.read().await;
        active.get(session_id).map(|s| s.write_half.clone())
    }

}

/// 服务端等待客户端加密协商
///
/// accept_pairing 发送 PairAccept 后，等待客户端发送 EncryptionNegotiate。
/// 加密为强制要求：如果对端不支持加密或协商失败，返回错误而非静默降级为明文。
async fn negotiate_encryption_server(
    pending: &PendingSession,
    _session_id: &str,
) -> Result<SecureKey, P2PError> {
    // 取出 read_half 用于读取协商消息
    let read_half_opt = {
        let mut rh = pending.read_half.lock().await;
        rh.take()
    };

    let mut reader = match read_half_opt {
        Some(r) => r,
        None => {
            return Err(P2PError::EncryptionNegotiationFailed(
                "No read_half available for encryption negotiation".to_string(),
            ));
        }
    };

    // 等待 EncryptionNegotiate 消息（带超时）
    let frame_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        ProtocolFrame::read_from_stream(&mut reader),
    )
    .await;

    let frame = match frame_result {
        Ok(Ok(f)) => f,
        Ok(Err(e)) => {
            // 放回 read_half
            let mut rh = pending.read_half.lock().await;
            *rh = Some(reader);
            return Err(P2PError::EncryptionNegotiationFailed(format!(
                "Error reading encryption negotiate: {}",
                e
            )));
        }
        Err(_) => {
            // 放回 read_half
            let mut rh = pending.read_half.lock().await;
            *rh = Some(reader);
            return Err(P2PError::EncryptionNegotiationFailed(
                "Encryption negotiation timed out".to_string(),
            ));
        }
    };

    // 如果收到的不是 EncryptionNegotiate，断开连接（加密为强制要求）
    if frame.msg_type != MSG_TYPE_ENCRYPTION_NEGOTIATE {
        // 放回 read_half
        let mut rh = pending.read_half.lock().await;
        *rh = Some(reader);
        return Err(P2PError::EncryptionNegotiationFailed(format!(
            "Expected encryption negotiate, got type {:#x}",
            frame.msg_type
        )));
    }

    let negotiate: EncryptionNegotiate = match decode_json(&frame) {
        Ok(n) => n,
        Err(e) => {
            let mut rh = pending.read_half.lock().await;
            *rh = Some(reader);
            return Err(P2PError::EncryptionNegotiationFailed(format!(
                "Failed to decode encryption negotiate: {}",
                e
            )));
        }
    };

    // 验证加密方法
    if negotiate.method != "aes-256-gcm" {
        let mut rh = pending.read_half.lock().await;
        *rh = Some(reader);
        return Err(P2PError::EncryptionNegotiationFailed(format!(
            "Unsupported encryption method: {}",
            negotiate.method
        )));
    }

    // 验证 salt 长度
    if negotiate.salt.len() != crypto::SALT_LEN {
        let mut rh = pending.read_half.lock().await;
        *rh = Some(reader);
        return Err(P2PError::EncryptionNegotiationFailed(format!(
            "Invalid salt length: expected {}, got {}",
            crypto::SALT_LEN,
            negotiate.salt.len()
        )));
    }

    // 验证 PBKDF2 迭代次数下限
    if negotiate.iterations < 100_000 {
        let mut rh = pending.read_half.lock().await;
        *rh = Some(reader);
        return Err(P2PError::EncryptionNegotiationFailed(format!(
            "PBKDF2 iterations too low: {}",
            negotiate.iterations
        )));
    }

    // 用配对码 + salt 派生密钥
    let key_bytes = crypto::derive_key(&pending.pair_code, &negotiate.salt);
    let key = SecureKey::new(key_bytes);

    // 生成验证 token（包含 client_nonce 防止预计算攻击）
    let verification =
        crypto::generate_verification(&negotiate.salt, &negotiate.client_nonce, key.as_bytes());

    // 发送 EncryptionConfirm
    let confirm = EncryptionConfirm {
        verification: verification.to_vec(),
    };
    let confirm_frame = match encode_encryption_confirm(&confirm) {
        Ok(f) => f,
        Err(e) => {
            let mut rh = pending.read_half.lock().await;
            *rh = Some(reader);
            return Err(P2PError::EncryptionNegotiationFailed(format!(
                "Failed to encode encryption confirm: {}",
                e
            )));
        }
    };

    {
        let mut wh = pending.write_half.lock().await;
        if let Err(e) = wh.write_all(&confirm_frame).await {
            let mut rh = pending.read_half.lock().await;
            *rh = Some(reader);
            return Err(P2PError::EncryptionNegotiationFailed(format!(
                "Failed to send encryption confirm: {}",
                e
            )));
        }
        if let Err(e) = wh.flush().await {
            let mut rh = pending.read_half.lock().await;
            *rh = Some(reader);
            return Err(P2PError::EncryptionNegotiationFailed(format!(
                "Failed to flush encryption confirm: {}",
                e
            )));
        }
    }

    tracing::info!("[P2P] Encryption negotiation completed successfully");

    // 放回 read_half 供后续 read_loop_inner 使用
    {
        let mut rh = pending.read_half.lock().await;
        *rh = Some(reader);
    }

    Ok(key)
}

/// 检查 IP 是否被速率限制
///
/// 返回 Ok(()) 表示可以继续，Err 表示被限制。
/// 使用指数退避：第 1 次失败后等 1s，第 2 次等 2s，以此类推（最大 30s）
fn check_rate_limit(attempt: &PairAttempt) -> Result<(), P2PError> {
    let elapsed = attempt.last_attempt_at.elapsed().as_secs();
    // 指数退避：失败次数对应等待秒数（最大 30s）
    let required_wait = std::cmp::min(2u64.saturating_pow(attempt.count.saturating_sub(1)), 30);

    if elapsed < required_wait {
        return Err(P2PError::RateLimited(format!(
            "Too many pairing attempts from this IP, please wait {} seconds",
            required_wait - elapsed
        )));
    }
    Ok(())
}

/// 记录配对失败
async fn record_pair_failure(
    peer_ip: &str,
    pair_attempts: &Arc<RwLock<HashMap<String, PairAttempt>>>,
) {
    let now = std::time::Instant::now();
    let mut attempts = pair_attempts.write().await;
    attempts
        .entry(peer_ip.to_string())
        .and_modify(|a| {
            a.count += 1;
            a.last_attempt_at = now;
        })
        .or_insert(PairAttempt {
            count: 1,
            first_attempt_at: now,
            last_attempt_at: now,
        });

    let count = attempts.get(peer_ip).map(|a| a.count).unwrap_or(0);
    tracing::warn!(
        "[P2P] Pair attempt failed for IP {}, total failures: {}",
        peer_ip,
        count
    );
}

/// 处理入站连接
async fn handle_incoming_connection(
    stream: TcpStream,
    addr: std::net::SocketAddr,
    expected_pair_code: &PairCode,
    event_tx: mpsc::UnboundedSender<P2PEvent>,
    known_sessions: Arc<RwLock<Vec<P2PSession>>>,
    pending_sessions: Arc<RwLock<HashMap<String, PendingSession>>>,
    active_sessions: Arc<RwLock<HashMap<String, ActiveSession>>>,
    pair_attempts: Arc<RwLock<HashMap<String, PairAttempt>>>,
) -> Result<(), P2PError> {
    let peer_ip = addr.ip().to_string();
    let peer_port = addr.port();

    // 拆分 stream 为读写两半
    let (mut read_half, write_half) = tokio::io::split(stream);
    let write_half = Arc::new(Mutex::new(write_half));

    let frame = ProtocolFrame::read_from_stream(&mut read_half).await?;
    if frame.msg_type == MSG_TYPE_TRANSFER_CHANNEL_OPEN {
        return handle_transfer_channel_connection(
            frame,
            read_half,
            write_half,
            peer_ip,
            active_sessions,
            event_tx,
        )
        .await;
    }

    // 检查速率限制
    {
        let attempts = pair_attempts.read().await;
        if let Some(attempt) = attempts.get(&peer_ip) {
            if attempt.count >= MAX_PAIR_ATTEMPTS {
                return Err(P2PError::RateLimited(format!(
                    "IP {} has exceeded maximum pairing attempts ({})",
                    peer_ip, MAX_PAIR_ATTEMPTS
                )));
            }
            check_rate_limit(attempt)?;
        }
    }

    if frame.msg_type != MSG_TYPE_PAIR_REQUEST {
        tracing::warn!(
            "[P2P] Expected pair request, got type {:#x}",
            frame.msg_type
        );
        return Err(P2PError::PairingFailed("Expected pair request".to_string()));
    }

    let pair_req: PairRequest = decode_json(&frame)?;

    // 验证配对码
    let received_code = pair_req.pair_code.to_uppercase();
    if received_code != expected_pair_code.as_str() {
        tracing::warn!(
            "[P2P] Pair code mismatch: expected={}, got={}",
            expected_pair_code,
            received_code
        );
        let reject = PairReject {
            reason: "Pair code mismatch".to_string(),
        };
        let reject_frame = encode_pair_reject(&reject)?;
        let mut wh = write_half.lock().await;
        let _ = wh.write_all(&reject_frame).await;
        let _ = wh.flush().await;

        // 记录失败
        record_pair_failure(&peer_ip, &pair_attempts).await;

        return Err(P2PError::PairCodeMismatch);
    }

    // 配对码匹配，创建待配对会话，通知前端
    let session_id = uuid::Uuid::new_v4().to_string();
    let device_name = pair_req.device_name.clone();

    tracing::info!(
        "[P2P] Pair request from {} (device: {}), session_id={}",
        peer_ip,
        device_name,
        session_id
    );

    // 将 read_half 包装为 Option 以便后续 accept_pairing 取出
    let read_half = Arc::new(Mutex::new(Some(read_half)));

    // 存入待配对会话
    let pending = PendingSession {
        read_half: read_half.clone(),
        write_half: write_half.clone(),
        peer_ip: peer_ip.clone(),
        peer_port,
        device_name: device_name.clone(),
        pair_code: received_code.clone(), // 保存配对码用于加密协商
        reconnect_ips: pair_req.listen_ips.clone(),
        reconnect_port: pair_req.listen_port,
    };

    let trusted_session = {
        let sessions = known_sessions.read().await;
        find_trusted_session(&sessions, &peer_ip, &device_name, &received_code)
    };
    if let Some(existing) = trusted_session {
        let (_key, session, _write_half) = accept_pending_session(
            pending,
            &existing.session_id,
            existing.session_id.clone(),
            event_tx.clone(),
            active_sessions,
            None,
        )
        .await?;
        {
            let mut sessions = known_sessions.write().await;
            if let Some(current) = sessions.iter_mut().find(|item| item.session_id == existing.session_id) {
                *current = session;
            }
        }
        return Ok(());
    }

    {
        let mut pending_map = pending_sessions.write().await;
        pending_map.insert(session_id.clone(), pending);
    }

    // 发送配对请求事件给前端
    let pairing_req = PairingRequest {
        session_id: session_id.clone(),
        peer_ip,
        peer_port,
        peer_device_name: device_name,
        pair_code: pair_req.pair_code,
        reconnect_ips: pair_req.listen_ips,
        reconnect_port: pair_req.listen_port,
    };
    let _ = event_tx.send(P2PEvent::PairingRequest(pairing_req));

    // 启动超时任务：如果 60 秒内未接受，自动拒绝并移除
    let timeout_session_id = session_id.clone();
    let timeout_pending = pending_sessions.clone();
    let timeout_write_half = write_half.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(
            PENDING_SESSION_TIMEOUT_SECS,
        ))
        .await;

        // 检查是否仍在待配对列表中
        let removed = {
            let mut pending_map = timeout_pending.write().await;
            pending_map.remove(&timeout_session_id).is_some()
        };

        if removed {
            tracing::warn!(
                "[P2P] Pending session {} timed out, auto-rejecting",
                timeout_session_id
            );
            let reject = PairReject {
                reason: "Pairing request timed out".to_string(),
            };
            if let Ok(reject_frame) = encode_pair_reject(&reject) {
                let mut wh = timeout_write_half.lock().await;
                let _ = wh.write_all(&reject_frame).await;
                let _ = wh.flush().await;
            }
        }
    });

    Ok(())
}

async fn handle_transfer_channel_connection(
    frame: ProtocolFrame,
    mut read_half: tokio::io::ReadHalf<TcpStream>,
    write_half: Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    peer_ip: String,
    active_sessions: Arc<RwLock<HashMap<String, ActiveSession>>>,
    event_tx: mpsc::UnboundedSender<P2PEvent>,
) -> Result<(), P2PError> {
    let open: TransferChannelOpen = decode_json(&frame)?;
    let session = {
        let active = active_sessions.read().await;
        active.get(&open.session_id).map(|s| ActiveSessionHandle {
            session_id: s.session_id.clone(),
            peer_ip: s.peer_ip.clone(),
        })
    }
    .ok_or_else(|| P2PError::SessionNotFound(open.session_id.clone()))?;

    let key_bytes = crypto::derive_key(&open.pair_code, &open.salt);
    let channel_key = SecureKey::new(key_bytes);

    if session.peer_ip != peer_ip {
        tracing::warn!(
            "[P2P] Transfer channel IP changed for session {}: {} -> {}",
            session.session_id,
            session.peer_ip,
            peer_ip
        );
    }

    let pending = take_pending_incoming_transfer(&session.session_id, &open.transfer_id)
        .await
        .ok_or_else(|| P2PError::TransferFailed("No pending incoming transfer".to_string()))?;

    let ready = TransferChannelReady {
        transfer_id: open.transfer_id.clone(),
    };
    let ready_frame = encode_frame_maybe_encrypted(
        MSG_TYPE_TRANSFER_CHANNEL_READY,
        &ready,
        Some(&channel_key),
    )?;
    {
        let mut wh = write_half.lock().await;
        wh.write_all(&ready_frame).await?;
        wh.flush().await?;
    }

    let mut received_data = Vec::with_capacity(pending.total_size as usize);
    let mut total_bytes = 0u64;
    loop {
        let frame = ProtocolFrame::read_from_stream(&mut read_half).await?;
        let payload = decrypt_frame_payload(&frame, Some(&channel_key))?;
        let decrypted = ProtocolFrame {
            msg_type: frame.msg_type,
            payload,
        };

        match decrypted.msg_type {
            MSG_TYPE_TRANSFER_DATA => {
                let data: TransferData = decode_json(&decrypted)?;
                received_data.extend_from_slice(&data.data);
                total_bytes += data.data.len() as u64;
                let percentage = if pending.total_size > 0 {
                    (total_bytes as f64 / pending.total_size as f64) * 100.0
                } else {
                    100.0
                };
                let _ = event_tx.send(P2PEvent::TransferProgress(TransferProgress {
                    session_id: session.session_id.clone(),
                    transfer_id: data.transfer_id,
                    bytes_transferred: total_bytes,
                    total_size: pending.total_size,
                    percentage,
                }));
            }
            MSG_TYPE_TRANSFER_END => {
                let end: TransferEnd = decode_json(&decrypted)?;
                let actual_checksum = compute_checksum(&received_data);
                let (success, reason) = if actual_checksum == end.checksum {
                    let _ = event_tx.send(P2PEvent::TransferComplete(TransferComplete {
                        session_id: session.session_id.clone(),
                        direction: "receive".to_string(),
                        bill_count: pending.bill_count,
                        bytes_transferred: total_bytes,
                    }));
                    let _ = event_tx.send(P2PEvent::DataReceived(DataReceived {
                        session_id: session.session_id.clone(),
                        data: received_data.clone(),
                    }));
                    (true, String::new())
                } else {
                    let reason = format!(
                        "Checksum mismatch: expected={}, actual={}",
                        end.checksum, actual_checksum
                    );
                    let _ = event_tx.send(P2PEvent::TransferError(TransferError {
                        session_id: session.session_id.clone(),
                        direction: "receive".to_string(),
                        error: reason.clone(),
                    }));
                    (false, reason)
                };

                let result = TransferChannelResult {
                    transfer_id: open.transfer_id.clone(),
                    success,
                    reason,
                };
                let result_frame = encode_frame_maybe_encrypted(
                    MSG_TYPE_TRANSFER_CHANNEL_RESULT,
                    &result,
                    Some(&channel_key),
                )?;
                let mut wh = write_half.lock().await;
                wh.write_all(&result_frame).await?;
                wh.flush().await?;
                return Ok(());
            }
            msg_type => {
                return Err(P2PError::TransferFailed(format!(
                    "Unexpected transfer channel message: {:#x}",
                    msg_type
                )));
            }
        }
    }
}

async fn accept_pending_session(
    pending: PendingSession,
    pending_session_id: &str,
    accepted_session_id: String,
    event_tx: mpsc::UnboundedSender<P2PEvent>,
    active_sessions: Arc<RwLock<HashMap<String, ActiveSession>>>,
    disconnect_tx: Option<mpsc::UnboundedSender<String>>,
) -> Result<
    (
        Option<SecureKey>,
        crate::P2PSession,
        Option<Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>>,
    ),
    P2PError,
> {
    let accept = PairAccept {
        device_name: "shmtu-terminal".to_string(),
        session_id: accepted_session_id.clone(),
    };
    let frame = encode_pair_accept(&accept)?;
    {
        let mut wh = pending.write_half.lock().await;
        wh.write_all(&frame).await?;
        wh.flush().await?;
    }

    let encryption_key = negotiate_encryption_server(&pending, &accepted_session_id).await?;

    let active_session = ActiveSession {
        write_half: pending.write_half.clone(),
        session_id: accepted_session_id.clone(),
        peer_ip: pending.peer_ip.clone(),
        peer_port: pending.peer_port,
        peer_device_name: pending.device_name.clone(),
        is_incoming: true,
        encryption_key: Some(encryption_key.clone()),
        pair_code: pending.pair_code.clone(),
        reconnect_ips: pending.reconnect_ips.clone(),
        reconnect_port: pending.reconnect_port,
    };

    let read_half = {
        let mut rh = pending.read_half.lock().await;
        rh.take()
    };

    if let Some(reader) = read_half {
        let stream = pending.write_half.clone();
        let read_session_id = accepted_session_id.clone();
        let enc_key = Some(encryption_key.clone());
        tokio::spawn(async move {
            read_loop_inner(
                reader,
                &read_session_id,
                event_tx,
                stream,
                enc_key,
                disconnect_tx.unwrap_or_else(|| {
                    let (tx, _rx) = mpsc::unbounded_channel();
                    tx
                }),
            )
            .await;
        });
    } else {
        tracing::warn!(
            "[P2P] No read_half available for session {}, read loop not started",
            accepted_session_id
        );
    }

    {
        let mut active = active_sessions.write().await;
        active.insert(accepted_session_id.clone(), active_session);
    }

    tracing::info!(
        "[P2P] Pairing accepted for pending session {}, session_id={}",
        pending_session_id,
        accepted_session_id
    );
    let session = crate::P2PSession {
        session_id: accepted_session_id.clone(),
        peer_ip: pending.peer_ip.clone(),
        peer_port: pending.peer_port,
        peer_device_name: pending.device_name.clone(),
        is_paired: true,
        is_incoming: true,
        is_connected: true,
        pair_code: Some(pending.pair_code.clone()),
        reconnect_ips: pending.reconnect_ips.clone(),
        reconnect_port: pending.reconnect_port,
    };
    Ok((Some(encryption_key), session, Some(pending.write_half.clone())))
}

fn find_trusted_session(
    sessions: &[P2PSession],
    peer_ip: &str,
    device_name: &str,
    pair_code: &str,
) -> Option<P2PSession> {
    sessions
        .iter()
        .filter(|session| {
            session.is_paired
                && !session.is_connected
                && session
                    .pair_code
                    .as_ref()
                    .map(|code| code.eq_ignore_ascii_case(pair_code))
                    .unwrap_or(false)
                && session.peer_device_name == device_name
        })
        .max_by_key(|session| {
            if session.peer_ip == peer_ip {
                3
            } else if session.reconnect_ips.iter().any(|ip| ip == peer_ip) {
                2
            } else {
                1
            }
        })
        .cloned()
}

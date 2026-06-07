pub mod client;
pub mod crypto;
pub mod discovery;
pub mod protocol;
pub mod server;
pub mod transfer;

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::WriteHalf;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::crypto::SecureKey;
use crate::server::P2PServer;
use crate::transfer::TransferProgress;

/// P2P 错误类型
#[derive(Debug, Error)]
pub enum P2PError {
    #[error("服务器未启动")]
    ServerNotStarted,

    #[error("服务器已在运行")]
    ServerAlreadyRunning,

    #[error("连接失败: {0}")]
    ConnectionFailed(String),

    #[error("配对失败: {0}")]
    PairingFailed(String),

    #[error("配对码不匹配")]
    PairCodeMismatch,

    #[error("配对速率受限: {0}")]
    RateLimited(String),

    #[error("会话不存在: {0}")]
    SessionNotFound(String),

    #[error("传输失败: {0}")]
    TransferFailed(String),

    #[error("校验和不匹配: 期望 {expected}, 实际 {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("加密失败: {0}")]
    EncryptionFailed(String),

    #[error("加密协商失败: {0}")]
    EncryptionNegotiationFailed(String),

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("协议错误: {0}")]
    Protocol(#[from] crate::protocol::ProtocolError),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("通道关闭")]
    ChannelClosed,
}

pub type P2PResult<T> = Result<T, P2PError>;

/// P2P 节点信息（前端展示用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PInfo {
    pub port: u16,
    pub pair_code: String,
    pub local_ips: Vec<String>,
    pub qr_payload: String,
    pub is_running: bool,
}

/// P2P 会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PSession {
    pub session_id: String,
    pub peer_ip: String,
    pub peer_port: u16,
    pub peer_device_name: String,
    pub is_paired: bool,
    pub is_incoming: bool,
    pub is_connected: bool,
    pub pair_code: Option<String>,
    pub reconnect_ips: Vec<String>,
    pub reconnect_port: Option<u16>,
}

/// P2P 整体状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PStatus {
    pub server_running: bool,
    pub port: u16,
    pub pair_code: String,
    pub local_ips: Vec<String>,
    pub sessions: Vec<P2PSession>,
}

/// 配对请求（服务端收到后通知前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    pub session_id: String,
    pub peer_ip: String,
    pub peer_port: u16,
    pub peer_device_name: String,
    pub pair_code: String,
    pub reconnect_ips: Vec<String>,
    pub reconnect_port: Option<u16>,
}

/// 传输完成通知
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferComplete {
    pub session_id: String,
    pub direction: String,
    pub bill_count: usize,
    pub bytes_transferred: u64,
}

/// 传输错误通知
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferError {
    pub session_id: String,
    pub direction: String,
    pub error: String,
}

/// 接收到的数据通知
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataReceived {
    pub session_id: String,
    pub data: Vec<u8>,
}

/// P2P 事件（通过通道发送给 Tauri 层，由 Tauri 层 emit 到前端）
#[derive(Debug, Clone)]
pub enum P2PEvent {
    /// 收到配对请求
    PairingRequest(PairingRequest),
    /// 传输进度
    TransferProgress(TransferProgress),
    /// 传输完成
    TransferComplete(TransferComplete),
    /// 传输错误
    TransferError(TransferError),
    /// 接收到完整数据
    DataReceived(DataReceived),
}

/// P2P 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PConfig {
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub auto_accept: bool,
    #[serde(default)]
    pub auto_reconnect: bool,
    #[serde(default = "default_device_name")]
    pub device_name: String,
    #[serde(default = "default_p2p_port")]
    pub port: u16,
}

fn default_device_name() -> String {
    "shmtu-terminal".to_string()
}

fn default_p2p_port() -> u16 {
    crate::protocol::P2P_DEFAULT_PORT
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            auto_start: false,
            auto_accept: false,
            auto_reconnect: false,
            device_name: default_device_name(),
            port: default_p2p_port(),
        }
    }
}

/// P2P 管理器：统一管理服务端、客户端和会话
pub struct P2PManager {
    inner: Arc<RwLock<P2PManagerInner>>,
    /// 事件接收器，可被取出一次供外部消费
    event_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<P2PEvent>>>>,
}

struct P2PManagerInner {
    server: Option<P2PServer>,
    sessions: HashMap<String, P2PSession>,
    known_sessions: Arc<RwLock<Vec<P2PSession>>>,
    event_tx: mpsc::UnboundedSender<P2PEvent>,
    disconnect_tx: mpsc::UnboundedSender<String>,
    /// 客户端连接的写半，用于 send_bills
    client_write_halves: HashMap<String, Arc<Mutex<WriteHalf<TcpStream>>>>,
    /// 每个 session 的加密密钥（SecureKey 在 drop 时自动清零）
    encryption_keys: HashMap<String, SecureKey>,
    config: P2PConfig,
}

impl P2PManager {
    /// 创建新的 P2P 管理器
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (disconnect_tx, mut disconnect_rx) = mpsc::unbounded_channel();
        let inner = Arc::new(RwLock::new(P2PManagerInner {
            server: None,
            sessions: HashMap::new(),
            known_sessions: Arc::new(RwLock::new(Vec::new())),
            event_tx,
            disconnect_tx: disconnect_tx.clone(),
            client_write_halves: HashMap::new(),
            encryption_keys: HashMap::new(),
            config: P2PConfig::default(),
        }));

        let inner_for_task = inner.clone();
        tokio::spawn(async move {
            while let Some(session_id) = disconnect_rx.recv().await {
                handle_connection_closed(inner_for_task.clone(), session_id).await;
            }
        });

        Self {
            inner,
            event_rx: Arc::new(Mutex::new(Some(event_rx))),
        }
    }

    /// 取出事件接收器（只能调用一次）
    pub async fn take_event_rx(&self) -> Option<mpsc::UnboundedReceiver<P2PEvent>> {
        let mut rx = self.event_rx.lock().await;
        rx.take()
    }

    /// 启动 P2P 服务端
    pub async fn start_server(&self, config: &P2PConfig) -> P2PResult<P2PInfo> {
        let mut inner = self.inner.write().await;
        if inner.server.is_some() {
            return Err(P2PError::ServerAlreadyRunning);
        }

        inner.config = config.clone();
        let event_tx = inner.event_tx.clone();
        let server = P2PServer::start(event_tx, config.port, inner.known_sessions.clone()).await?;
        let info = server.info();

        inner.server = Some(server);
        Ok(info)
    }

    /// 停止 P2P 服务端
    pub async fn stop_server(&self) -> P2PResult<()> {
        let mut inner = self.inner.write().await;
        if let Some(server) = inner.server.take() {
            server.stop().await?;
            inner.sessions.clear();
            inner.client_write_halves.clear();
            // SecureKey 的 Drop 会自动清零每个密钥
            inner.encryption_keys.clear();
            tracing::info!("[P2P] Server stopped and all sessions cleared");
        }
        Ok(())
    }

    /// 获取 QR 码载荷
    pub async fn get_qr_payload(&self) -> P2PResult<String> {
        let inner = self.inner.read().await;
        let server = inner.server.as_ref().ok_or(P2PError::ServerNotStarted)?;
        Ok(server.qr_payload())
    }

    /// 获取 P2P 信息
    pub async fn get_info(&self) -> P2PResult<P2PInfo> {
        let inner = self.inner.read().await;
        let server = inner.server.as_ref().ok_or(P2PError::ServerNotStarted)?;
        Ok(server.info())
    }

    /// 发起连接（连接到远端服务端）
    pub async fn connect(
        &self,
        addr: String,
        port: u16,
        pair_code: String,
        device_name: String,
    ) -> P2PResult<P2PSession> {
        let inner = self.inner.read().await;
        let event_tx = inner.event_tx.clone();
        let disconnect_tx = inner.disconnect_tx.clone();
        let listen_port = inner
            .server
            .as_ref()
            .map(|server| server.info().port)
            .unwrap_or(inner.config.port);
        drop(inner);

        let (session, write_half, encryption_key) =
            crate::client::P2PClient::connect(
                addr,
                port,
                pair_code,
                device_name,
                event_tx,
                disconnect_tx,
                Some(listen_port),
                crate::discovery::get_local_ips(),
            )
            .await?;

        let session_clone = session.clone();
        let mut inner = self.inner.write().await;
        inner
            .sessions
            .insert(session.session_id.clone(), session_clone);
        upsert_known_session(&inner.known_sessions, session.clone()).await;
        inner
            .client_write_halves
            .insert(session.session_id.clone(), write_half);
        if let Some(key) = encryption_key {
            inner
                .encryption_keys
                .insert(session.session_id.clone(), key);
        }
        Ok(session)
    }

    /// 接受配对请求
    pub async fn accept_pairing(&self, session_id: &str) -> P2PResult<()> {
        let (encryption_key, active_session, write_half) = {
            let inner = self.inner.read().await;
            let server = inner.server.as_ref().ok_or(P2PError::ServerNotStarted)?;
            server
                .accept_pairing_with_session(session_id, Some(inner.disconnect_tx.clone()))
                .await?
        };
        let mut inner = self.inner.write().await;
        inner
            .sessions
            .insert(active_session.session_id.clone(), active_session.clone());
        upsert_known_session(&inner.known_sessions, active_session.clone()).await;
        if let Some(wh) = write_half {
            inner
                .client_write_halves
                .insert(active_session.session_id.clone(), wh);
        }
        if let Some(key) = encryption_key {
            inner
                .encryption_keys
                .insert(active_session.session_id.clone(), key);
        }
        Ok(())
    }

    /// 拒绝配对请求
    pub async fn reject_pairing(&self, session_id: &str) -> P2PResult<()> {
        let inner = self.inner.read().await;
        if let Some(server) = &inner.server {
            server.reject_pairing(session_id).await
        } else {
            Err(P2PError::ServerNotStarted)
        }
    }

    /// 发送账单数据给对端
    pub async fn send_bills(&self, session_id: &str, data: &[u8]) -> P2PResult<()> {
        let inner = self.inner.read().await;

        // 先尝试从服务端发送
        if let Some(server) = &inner.server {
            if let Ok(()) = server.send_data(session_id, data).await {
                return Ok(());
            }
        }

        // 服务端找不到则尝试从客户端连接发送
        let write_half = inner.client_write_halves.get(session_id).cloned();
        let encryption_key = inner.encryption_keys.get(session_id).cloned();
        let session = inner.sessions.get(session_id).cloned();
        drop(inner);

        if let Some(wh) = write_half {
            let session = session.ok_or_else(|| P2PError::SessionNotFound(session_id.to_string()))?;
            let pair_code = session
                .pair_code
                .clone()
                .ok_or_else(|| P2PError::PairingFailed("当前会话缺少配对码".to_string()))?;
            let engine = crate::transfer::TransferEngine::new(
                wh,
                session.peer_ip.clone(),
                session.peer_port,
                session.session_id.clone(),
                pair_code,
                encryption_key,
            );
            let transfer_id = uuid::Uuid::new_v4().to_string();

            // 获取 event_tx
            let inner = self.inner.read().await;
            let event_tx = inner.event_tx.clone();
            drop(inner);

            engine.send(data, &transfer_id, event_tx, session_id).await
        } else {
            Err(P2PError::SessionNotFound(session_id.to_string()))
        }
    }

    /// 获取 P2P 整体状态
    pub async fn get_status(&self) -> P2PStatus {
        let inner = self.inner.read().await;
        let (server_running, port, pair_code, local_ips) = if let Some(server) = &inner.server {
            let info = server.info();
            (true, info.port, info.pair_code, info.local_ips)
        } else {
            (false, 0, String::new(), Vec::new())
        };
        let sessions: Vec<P2PSession> = inner.sessions.values().cloned().collect();
        P2PStatus {
            server_running,
            port,
            pair_code,
            local_ips,
            sessions,
        }
    }

    /// 断开会话
    pub async fn disconnect(&self, session_id: &str) -> P2PResult<()> {
        let mut inner = self.inner.write().await;
        let is_connected = inner
            .sessions
            .get(session_id)
            .map(|session| session.is_connected)
            .unwrap_or(false);

        if !is_connected {
            inner.sessions.remove(session_id);
            remove_known_session(&inner.known_sessions, session_id).await;
            inner.client_write_halves.remove(session_id);
            inner.encryption_keys.remove(session_id);
            return Ok(());
        }

        if let Some(server) = &inner.server {
            let _ = server.disconnect(session_id).await;
        }
        let known_sessions = inner.known_sessions.clone();
        if let Some(session) = inner.sessions.get_mut(session_id) {
            session.is_connected = false;
            let updated = session.clone();
            let _ = session;
            upsert_known_session(&known_sessions, updated).await;
        }
        inner.client_write_halves.remove(session_id);
        // SecureKey 的 Drop 会在 remove 时自动清零密钥
        inner.encryption_keys.remove(session_id);
        Ok(())
    }

    /// 手动配对（等价于 connect，提供更语义化的名称）
    pub async fn manual_pair(
        &self,
        ip: String,
        port: u16,
        pair_code: String,
        device_name: String,
    ) -> P2PResult<P2PSession> {
        self.connect(ip, port, pair_code, device_name).await
    }

    /// 注册已配对的会话（服务端收到配对成功后调用）
    pub async fn register_session(&self, session: P2PSession) {
        let mut inner = self.inner.write().await;
        inner.sessions.insert(session.session_id.clone(), session);
        if let Some(session) = inner.sessions.values().last().cloned() {
            upsert_known_session(&inner.known_sessions, session).await;
        }
    }

    /// 注册服务端会话的写半
    pub async fn register_server_write_half(
        &self,
        session_id: String,
        write_half: Arc<Mutex<WriteHalf<TcpStream>>>,
    ) {
        let mut inner = self.inner.write().await;
        inner.client_write_halves.insert(session_id, write_half);
    }

    /// 移除会话
    pub async fn remove_session(&self, session_id: &str) {
        let mut inner = self.inner.write().await;
        inner.sessions.remove(session_id);
        remove_known_session(&inner.known_sessions, session_id).await;
        inner.client_write_halves.remove(session_id);
        // SecureKey 的 Drop 会在 remove 时自动清零密钥
        inner.encryption_keys.remove(session_id);
    }

    pub async fn reconnect(&self, session_id: &str) -> P2PResult<P2PSession> {
        let existing = {
            let inner = self.inner.read().await;
            inner
                .sessions
                .get(session_id)
                .cloned()
                .ok_or_else(|| P2PError::SessionNotFound(session_id.to_string()))?
        };

        if existing.is_connected {
            return Ok(existing);
        }

        let pair_code = existing
            .pair_code
            .clone()
            .ok_or_else(|| P2PError::PairingFailed("当前会话缺少配对码".to_string()))?;

        let (target_ip, target_port) = if existing.is_incoming {
            let port = existing
                .reconnect_port
                .ok_or_else(|| P2PError::ConnectionFailed("当前会话缺少重连端口".to_string()))?;
            let ip = select_reconnect_ip(&existing.reconnect_ips)
                .ok_or_else(|| P2PError::ConnectionFailed("当前会话缺少重连地址".to_string()))?;
            (ip, port)
        } else {
            (existing.peer_ip.clone(), existing.peer_port)
        };

        let inner = self.inner.read().await;
        let event_tx = inner.event_tx.clone();
        let disconnect_tx = inner.disconnect_tx.clone();
        let device_name = inner.config.device_name.clone();
        let listen_port = inner
            .server
            .as_ref()
            .map(|server| server.info().port)
            .unwrap_or(inner.config.port);
        drop(inner);

        let (fresh_session, write_half, encryption_key) = crate::client::P2PClient::connect(
            target_ip.clone(),
            target_port,
            pair_code.clone(),
            device_name,
            event_tx,
            disconnect_tx,
            Some(listen_port),
            crate::discovery::get_local_ips(),
        )
        .await?;

        let updated = P2PSession {
            session_id: session_id.to_string(),
            peer_ip: target_ip,
            peer_port: target_port,
            peer_device_name: fresh_session.peer_device_name,
            is_paired: true,
            is_incoming: existing.is_incoming,
            is_connected: true,
            pair_code: Some(pair_code),
            reconnect_ips: existing.reconnect_ips,
            reconnect_port: existing.reconnect_port,
        };

        let mut inner = self.inner.write().await;
        inner.sessions.insert(session_id.to_string(), updated.clone());
        upsert_known_session(&inner.known_sessions, updated.clone()).await;
        inner
            .client_write_halves
            .insert(session_id.to_string(), write_half);
        if let Some(key) = encryption_key {
            inner.encryption_keys.insert(session_id.to_string(), key);
        }
        Ok(updated)
    }
}

fn select_reconnect_ip(ips: &[String]) -> Option<String> {
    ips.iter()
        .find(|ip| !ip.starts_with("127."))
        .cloned()
        .or_else(|| ips.first().cloned())
}

async fn handle_connection_closed(inner: Arc<RwLock<P2PManagerInner>>, session_id: String) {
    let should_reconnect = {
        let mut inner_guard = inner.write().await;
        let auto_reconnect = inner_guard.config.auto_reconnect;
        let known_sessions = inner_guard.known_sessions.clone();
        inner_guard.client_write_halves.remove(&session_id);
        inner_guard.encryption_keys.remove(&session_id);
        if let Some(session) = inner_guard.sessions.get_mut(&session_id) {
            session.is_connected = false;
            let updated = session.clone();
            let should_reconnect = auto_reconnect
                && updated.is_paired
                && updated.pair_code.is_some()
                && (!updated.is_incoming || (updated.reconnect_port.is_some() && !updated.reconnect_ips.is_empty()));
            upsert_known_session(&known_sessions, updated).await;
            should_reconnect
        } else {
            false
        }
    };

    if should_reconnect {
        let inner_for_retry = inner.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            let _ = reconnect_with_inner(inner_for_retry, &session_id).await;
        });
    }
}

async fn reconnect_with_inner(
    inner: Arc<RwLock<P2PManagerInner>>,
    session_id: &str,
) -> P2PResult<P2PSession> {
    let existing = {
        let inner = inner.read().await;
        inner
            .sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| P2PError::SessionNotFound(session_id.to_string()))?
    };

    if existing.is_connected {
        return Ok(existing);
    }

    let pair_code = existing
        .pair_code
        .clone()
        .ok_or_else(|| P2PError::PairingFailed("当前会话缺少配对码".to_string()))?;

    let (target_ip, target_port) = if existing.is_incoming {
        let port = existing
            .reconnect_port
            .ok_or_else(|| P2PError::ConnectionFailed("当前会话缺少重连端口".to_string()))?;
        let ip = select_reconnect_ip(&existing.reconnect_ips)
            .ok_or_else(|| P2PError::ConnectionFailed("当前会话缺少重连地址".to_string()))?;
        (ip, port)
    } else {
        (existing.peer_ip.clone(), existing.peer_port)
    };

    let inner_guard = inner.read().await;
    let event_tx = inner_guard.event_tx.clone();
    let disconnect_tx = inner_guard.disconnect_tx.clone();
    let device_name = inner_guard.config.device_name.clone();
    let listen_port = inner_guard
        .server
        .as_ref()
        .map(|server| server.info().port)
        .unwrap_or(inner_guard.config.port);
    drop(inner_guard);

    let (fresh_session, write_half, encryption_key) = crate::client::P2PClient::connect(
        target_ip.clone(),
        target_port,
        pair_code.clone(),
        device_name,
        event_tx,
        disconnect_tx,
        Some(listen_port),
        crate::discovery::get_local_ips(),
    )
    .await?;

    let updated = P2PSession {
        session_id: session_id.to_string(),
        peer_ip: target_ip,
        peer_port: target_port,
        peer_device_name: fresh_session.peer_device_name,
        is_paired: true,
        is_incoming: existing.is_incoming,
        is_connected: true,
        pair_code: Some(pair_code),
        reconnect_ips: existing.reconnect_ips,
        reconnect_port: existing.reconnect_port,
    };

    let mut inner_guard = inner.write().await;
    inner_guard
        .sessions
        .insert(session_id.to_string(), updated.clone());
    upsert_known_session(&inner_guard.known_sessions, updated.clone()).await;
    inner_guard
        .client_write_halves
        .insert(session_id.to_string(), write_half);
    if let Some(key) = encryption_key {
        inner_guard
            .encryption_keys
            .insert(session_id.to_string(), key);
    }
    Ok(updated)
}

impl Default for P2PManager {
    fn default() -> Self {
        Self::new()
    }
}

async fn upsert_known_session(store: &Arc<RwLock<Vec<P2PSession>>>, session: P2PSession) {
    let mut sessions = store.write().await;
    if let Some(existing) = sessions.iter_mut().find(|item| item.session_id == session.session_id) {
        *existing = session;
    } else {
        sessions.push(session);
    }
}

async fn remove_known_session(store: &Arc<RwLock<Vec<P2PSession>>>, session_id: &str) {
    let mut sessions = store.write().await;
    sessions.retain(|session| session.session_id != session_id);
}

use std::collections::HashMap;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::crypto::{self, SecureKey};
use crate::protocol::*;
use crate::transfer::{compute_checksum, TransferProgress};
use crate::{DataReceived, P2PError, P2PEvent, P2PSession, TransferComplete, TransferError};

/// 加密协商超时时间（秒）
const ENCRYPTION_NEGOTIATION_TIMEOUT_SECS: u64 = 10;

/// P2P 客户端
pub struct P2PClient;

impl P2PClient {
    /// 连接到远端 P2P 服务端并完成配对和加密协商
    /// 返回会话信息、写半部分和加密密钥
    pub async fn connect(
        addr: String,
        port: u16,
        pair_code: String,
        device_name: String,
        event_tx: mpsc::UnboundedSender<P2PEvent>,
    ) -> Result<
        (
            P2PSession,
            Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
            Option<SecureKey>,
        ),
        P2PError,
    > {
        let target = format!("{}:{}", addr, port);
        tracing::info!("[P2P] Connecting to {}", target);

        let stream = TcpStream::connect(&target).await.map_err(|e| {
            P2PError::ConnectionFailed(format!("Failed to connect to {}: {}", target, e))
        })?;

        tracing::info!("[P2P] Connected to {}, sending pair request", target);

        // 拆分 stream 为读写两半
        let (read_half, write_half) = tokio::io::split(stream);
        let write_half = Arc::new(Mutex::new(write_half));

        // 发送配对请求
        let pair_req = PairRequest {
            pair_code: pair_code.to_uppercase(),
            device_name: device_name.clone(),
        };
        let req_frame = encode_pair_request(&pair_req)?;
        {
            let mut wh = write_half.lock().await;
            wh.write_all(&req_frame).await?;
            wh.flush().await?;
        }

        // 在读半上等待响应
        let mut reader = read_half;
        let response_frame = ProtocolFrame::read_from_stream(&mut reader).await?;

        match response_frame.msg_type {
            MSG_TYPE_PAIR_ACCEPT => {
                let accept: PairAccept = decode_json(&response_frame)?;
                let session_id = accept.session_id.clone();
                let peer_device_name = accept.device_name.clone();

                tracing::info!(
                    "[P2P] Pairing accepted by {}, session_id={}",
                    target,
                    session_id
                );

                // ---- 加密协商 ----
                let encryption_key =
                    negotiate_encryption_client(&mut reader, &write_half, &pair_code).await?;

                if encryption_key.is_some() {
                    tracing::info!("[P2P] Encryption established for session {}", session_id);
                } else {
                    tracing::warn!(
                        "[P2P] Encryption negotiation failed for session {}, disconnecting (encryption is mandatory)",
                        session_id
                    );
                    return Err(P2PError::EncryptionNegotiationFailed(
                        "Peer does not support encryption, but encryption is mandatory".to_string(),
                    ));
                }

                let session = P2PSession {
                    session_id: session_id.clone(),
                    peer_ip: addr.clone(),
                    peer_port: port,
                    peer_device_name,
                    is_paired: true,
                    is_incoming: false,
                };

                // 启动消息读取循环（使用读半）
                let event_tx_clone = event_tx.clone();
                let session_id_clone = session_id.clone();
                let write_half_clone = write_half.clone();
                let encryption_key_clone = encryption_key.clone();
                tokio::spawn(async move {
                    read_loop_inner(
                        reader,
                        &session_id_clone,
                        event_tx_clone,
                        write_half_clone,
                        encryption_key_clone,
                    )
                    .await;
                });

                Ok((session, write_half, encryption_key))
            }
            MSG_TYPE_PAIR_REJECT => {
                let reject: PairReject = decode_json(&response_frame)?;
                tracing::warn!("[P2P] Pairing rejected by {}: {}", target, reject.reason);
                Err(P2PError::PairingFailed(reject.reason))
            }
            msg_type => {
                tracing::warn!(
                    "[P2P] Unexpected response type {:#x} from {}",
                    msg_type,
                    target
                );
                Err(P2PError::PairingFailed(format!(
                    "Unexpected response type: {:#x}",
                    msg_type
                )))
            }
        }
    }
}

/// 客户端发起加密协商
///
/// 返回 Ok(Some(key)) 表示加密已建立，
/// Ok(None) 表示对端不支持加密协商（加密为强制，调用方应断开连接）。
async fn negotiate_encryption_client<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    write_half: &Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    pair_code: &str,
) -> Result<Option<SecureKey>, P2PError> {
    let pair_code_upper = pair_code.to_uppercase();

    // 生成随机 salt
    let salt = crypto::generate_salt();
    let client_nonce: [u8; 8] = rand::random();

    // 从配对码派生密钥
    let key_bytes = crypto::derive_key(&pair_code_upper, &salt);
    let key = SecureKey::new(key_bytes);

    // 构建 EncryptionNegotiate 消息
    let negotiate = EncryptionNegotiate {
        method: "aes-256-gcm".to_string(),
        salt: salt.to_vec(),
        iterations: crypto::PBKDF2_ITERATIONS,
        client_nonce: client_nonce.to_vec(),
    };

    let neg_frame = encode_encryption_negotiate(&negotiate)?;
    {
        let mut wh = write_half.lock().await;
        wh.write_all(&neg_frame).await?;
        wh.flush().await?;
    }

    // 等待 EncryptionConfirm 回复（带超时）
    let confirm_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(ENCRYPTION_NEGOTIATION_TIMEOUT_SECS),
        ProtocolFrame::read_from_stream(reader),
    )
    .await;

    match confirm_result {
        Ok(Ok(frame)) => {
            if frame.msg_type == MSG_TYPE_ENCRYPTION_CONFIRM {
                // 验证 verification token
                let confirm: EncryptionConfirm = decode_json(&frame)?;
                let expected_verification =
                    crypto::generate_verification(&salt, &client_nonce, key.as_bytes());

                if confirm.verification.len() == crypto::VERIFICATION_LEN
                    && confirm.verification.as_slice() == expected_verification
                {
                    tracing::info!("[P2P] Encryption negotiation verified successfully");
                    Ok(Some(key))
                } else {
                    tracing::error!("[P2P] Encryption verification failed: token mismatch");
                    Err(P2PError::EncryptionNegotiationFailed(
                        "Verification token mismatch".to_string(),
                    ))
                }
            } else {
                // 对端回复了其他消息类型，说明不支持加密协商
                // 加密为强制要求，返回错误而非静默降级
                tracing::warn!(
                    "[P2P] Peer responded with unexpected message type {:#x} during encryption negotiation",
                    frame.msg_type
                );
                Err(P2PError::EncryptionNegotiationFailed(
                    format!(
                        "Expected encryption confirm, got type {:#x}",
                        frame.msg_type
                    ),
                ))
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(
                "[P2P] Error reading encryption confirm: {}",
                e
            );
            Err(P2PError::EncryptionNegotiationFailed(format!(
                "Error reading encryption confirm: {}",
                e
            )))
        }
        Err(_) => {
            // 超时，对端可能不支持加密
            tracing::warn!("[P2P] Encryption negotiation timed out");
            Err(P2PError::EncryptionNegotiationFailed(
                "Encryption negotiation timed out".to_string(),
            ))
        }
    }
}

/// 等待 TransferAccept 的 oneshot 通道注册表
type AcceptWaiters = Arc<Mutex<HashMap<String, oneshot::Sender<Result<(), P2PError>>>>>;

/// 全局等待表（由 read_loop_inner 和 TransferEngine 共享）
static ACCEPT_WAITERS: std::sync::OnceLock<AcceptWaiters> = std::sync::OnceLock::new();

fn get_accept_waiters() -> AcceptWaiters {
    ACCEPT_WAITERS
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

/// 注册一个 TransferAccept 等待者
pub async fn register_accept_waiter(
    transfer_id: String,
    tx: oneshot::Sender<Result<(), P2PError>>,
) {
    let waiters = get_accept_waiters();
    let mut map = waiters.lock().await;
    map.insert(transfer_id, tx);
}

/// 消息读取循环：从已配对的连接持续读取消息
/// 此函数同时被客户端和服务端（accept_pairing 后）调用
pub async fn read_loop_inner<R: AsyncReadExt + Unpin + Send + 'static>(
    mut reader: R,
    session_id: &str,
    event_tx: mpsc::UnboundedSender<P2PEvent>,
    write_half: Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    encryption_key: Option<SecureKey>,
) {
    tracing::debug!(
        "[P2P] Read loop started for session {} (encrypted: {})",
        session_id,
        encryption_key.is_some()
    );

    let mut received_data: Vec<u8> = Vec::new();
    let mut expected_total: u64 = 0;
    let mut total_bytes: u64 = 0;

    // 启动心跳定时任务
    let heartbeat_session_id = session_id.to_string();
    let heartbeat_write = write_half.clone();
    let (heartbeat_shutdown_tx, mut heartbeat_shutdown_rx) = mpsc::unbounded_channel::<()>();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let ping_frame = match encode_ping() {
                        Ok(f) => f,
                        Err(e) => {
                            tracing::warn!("[P2P] Failed to encode ping: {}", e);
                            continue;
                        }
                    };
                    let mut wh = heartbeat_write.lock().await;
                    if let Err(e) = wh.write_all(&ping_frame).await {
                        tracing::warn!("[P2P] Failed to send ping for session {}: {}", heartbeat_session_id, e);
                        break;
                    }
                    if let Err(e) = wh.flush().await {
                        tracing::warn!("[P2P] Failed to flush ping for session {}: {}", heartbeat_session_id, e);
                        break;
                    }
                }
                _ = heartbeat_shutdown_rx.recv() => {
                    tracing::debug!("[P2P] Heartbeat task shutting down for session {}", heartbeat_session_id);
                    break;
                }
            }
        }
    });

    loop {
        let frame = match ProtocolFrame::read_from_stream(&mut reader).await {
            Ok(frame) => frame,
            Err(ProtocolError::ConnectionClosed) => {
                tracing::info!("[P2P] Connection closed for session {}", session_id);
                break;
            }
            Err(e) => {
                tracing::warn!("[P2P] Read error for session {}: {}", session_id, e);
                break;
            }
        };

        // 使用统一的帧解密函数
        let decrypted_payload = match decrypt_frame_payload(&frame, encryption_key.as_ref()) {
            Ok(payload) => payload,
            Err(e) => {
                tracing::error!(
                    "[P2P] Decryption failed for msg type {:#x} in session {}: {}",
                    frame.msg_type,
                    session_id,
                    e
                );
                // 解密失败，断开连接
                break;
            }
        };

        // 构建解密后的帧用于后续处理
        let decrypted_frame = ProtocolFrame {
            msg_type: frame.msg_type,
            payload: decrypted_payload,
        };

        match decrypted_frame.msg_type {
            MSG_TYPE_PING => {
                // 收到 PING，回复 PONG
                let pong_frame = match encode_pong() {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::error!("[P2P] Failed to encode pong: {}", e);
                        break;
                    }
                };
                let mut wh = write_half.lock().await;
                if let Err(e) = wh.write_all(&pong_frame).await {
                    tracing::error!("[P2P] Failed to send pong: {}", e);
                    break;
                }
                if let Err(e) = wh.flush().await {
                    tracing::error!("[P2P] Failed to flush pong: {}", e);
                    break;
                }
            }
            MSG_TYPE_PONG => {
                // 收到 PONG 回复，无需处理
                tracing::trace!("[P2P] Received pong for session {}", session_id);
            }
            MSG_TYPE_TRANSFER_OFFER => {
                match decode_json::<TransferOffer>(&decrypted_frame) {
                    Ok(offer) => {
                        tracing::info!(
                            "[P2P] Received transfer offer: {} ({} bytes)",
                            offer.description,
                            offer.total_size
                        );
                        expected_total = offer.total_size;
                        total_bytes = 0;
                        received_data.clear();

                        // 自动接受传输
                        let accept = TransferAccept {
                            transfer_id: offer.transfer_id.clone(),
                        };
                        let accept_frame = match encode_frame_maybe_encrypted(
                            MSG_TYPE_TRANSFER_ACCEPT,
                            &accept,
                            encryption_key.as_ref(),
                        ) {
                            Ok(f) => f,
                            Err(e) => {
                                tracing::error!("[P2P] Failed to encode transfer accept: {}", e);
                                break;
                            }
                        };
                        let mut wh = write_half.lock().await;
                        if let Err(e) = wh.write_all(&accept_frame).await {
                            tracing::error!("[P2P] Failed to send transfer accept: {}", e);
                            break;
                        }
                        if let Err(e) = wh.flush().await {
                            tracing::error!("[P2P] Failed to flush: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("[P2P] Failed to decode transfer offer: {}", e);
                    }
                }
            }
            MSG_TYPE_TRANSFER_ACCEPT => {
                // TransferAccept 响应，通知等待的 send 操作
                match decode_json::<TransferAccept>(&decrypted_frame) {
                    Ok(accept) => {
                        let waiters = get_accept_waiters();
                        let mut map = waiters.lock().await;
                        if let Some(tx) = map.remove(&accept.transfer_id) {
                            let _ = tx.send(Ok(()));
                        }
                    }
                    Err(e) => {
                        tracing::error!("[P2P] Failed to decode transfer accept: {}", e);
                    }
                }
            }
            MSG_TYPE_TRANSFER_REJECT => {
                // TransferReject 响应，通知等待的 send 操作
                match decode_json::<TransferReject>(&decrypted_frame) {
                    Ok(reject) => {
                        let waiters = get_accept_waiters();
                        let mut map = waiters.lock().await;
                        if let Some(tx) = map.remove(&reject.transfer_id) {
                            let _ = tx.send(Err(P2PError::TransferFailed(format!(
                                "Transfer rejected: {}",
                                reject.reason
                            ))));
                        }
                    }
                    Err(e) => {
                        tracing::error!("[P2P] Failed to decode transfer reject: {}", e);
                    }
                }
            }
            MSG_TYPE_TRANSFER_DATA => {
                match decode_json::<TransferData>(&decrypted_frame) {
                    Ok(data) => {
                        received_data.extend_from_slice(&data.data);
                        total_bytes += data.data.len() as u64;

                        // 计算进度百分比
                        let percentage = if expected_total > 0 {
                            (total_bytes as f64 / expected_total as f64) * 100.0
                        } else {
                            0.0
                        };

                        // 发送进度事件
                        let progress = TransferProgress {
                            session_id: session_id.to_string(),
                            transfer_id: data.transfer_id.clone(),
                            bytes_transferred: total_bytes,
                            total_size: expected_total,
                            percentage,
                        };
                        let _ = event_tx.send(P2PEvent::TransferProgress(progress));
                    }
                    Err(e) => {
                        tracing::error!("[P2P] Failed to decode transfer data: {}", e);
                    }
                }
            }
            MSG_TYPE_TRANSFER_END => {
                match decode_json::<TransferEnd>(&decrypted_frame) {
                    Ok(end) => {
                        tracing::info!(
                            "[P2P] Transfer complete: {} bytes, checksum={}",
                            total_bytes,
                            end.checksum
                        );

                        // 验证校验和
                        let actual_checksum = compute_checksum(&received_data);
                        if actual_checksum != end.checksum {
                            tracing::error!(
                                "[P2P] Checksum mismatch: expected={}, actual={}",
                                end.checksum,
                                actual_checksum
                            );
                            let error = TransferError {
                                session_id: session_id.to_string(),
                                direction: "receive".to_string(),
                                error: format!(
                                    "Checksum mismatch: expected={}, actual={}",
                                    end.checksum, actual_checksum
                                ),
                            };
                            let _ = event_tx.send(P2PEvent::TransferError(error));
                            // 重置传输状态
                            total_bytes = 0;
                            expected_total = 0;
                            received_data.clear();
                            continue;
                        }

                        // 解析账单数据计算数量
                        let bill_count =
                            serde_json::from_slice::<serde_json::Value>(&received_data)
                                .ok()
                                .and_then(|v| {
                                    v.get("bills").and_then(|b| b.as_array()).map(|a| a.len())
                                })
                                .unwrap_or(0);

                        let complete = TransferComplete {
                            session_id: session_id.to_string(),
                            direction: "receive".to_string(),
                            bill_count,
                            bytes_transferred: total_bytes,
                        };
                        let _ = event_tx.send(P2PEvent::TransferComplete(complete));

                        // 发送 DataReceived 事件，携带完整数据
                        let data_received = DataReceived {
                            session_id: session_id.to_string(),
                            data: received_data.clone(),
                        };
                        let _ = event_tx.send(P2PEvent::DataReceived(data_received));

                        // 重置传输状态
                        total_bytes = 0;
                        expected_total = 0;
                        received_data.clear();
                    }
                    Err(e) => {
                        tracing::error!("[P2P] Failed to decode transfer end: {}", e);
                    }
                }
            }
            MSG_TYPE_DISCONNECT => {
                tracing::info!("[P2P] Peer disconnected session {}", session_id);
                break;
            }
            msg_type => {
                tracing::warn!(
                    "[P2P] Unknown message type {:#x} in session {}",
                    msg_type,
                    session_id
                );
            }
        }
    }

    // 关闭心跳任务
    let _ = heartbeat_shutdown_tx.send(());

    tracing::debug!("[P2P] Read loop ended for session {}", session_id);
}

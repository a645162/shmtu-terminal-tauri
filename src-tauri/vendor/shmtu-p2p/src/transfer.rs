use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};

use crate::client::register_accept_waiter;
use crate::crypto::{self, SecureKey};
use crate::protocol::*;
use crate::{P2PError, P2PEvent, TransferComplete, TransferError};

/// 传输进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub session_id: String,
    pub transfer_id: String,
    pub bytes_transferred: u64,
    pub total_size: u64,
    pub percentage: f64,
}

/// 传输引擎
pub struct TransferEngine {
    write_half: Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    target_ip: String,
    target_port: u16,
    session_id: String,
    pair_code: String,
    /// 加密密钥（可选）
    encryption_key: Option<SecureKey>,
}

#[derive(Debug, Clone)]
pub struct PendingIncomingTransfer {
    pub session_id: String,
    pub transfer_id: String,
    pub total_size: u64,
    pub bill_count: usize,
}

/// 数据块大小 (64KB)
const CHUNK_SIZE: usize = 64 * 1024;

type PendingIncomingTransfers = Arc<Mutex<HashMap<String, PendingIncomingTransfer>>>;

static PENDING_INCOMING_TRANSFERS: std::sync::OnceLock<PendingIncomingTransfers> =
    std::sync::OnceLock::new();

fn get_pending_incoming_transfers() -> PendingIncomingTransfers {
    PENDING_INCOMING_TRANSFERS
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

impl TransferEngine {
    /// 从写半创建传输引擎
    pub fn new(
        write_half: Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
        target_ip: String,
        target_port: u16,
        session_id: String,
        pair_code: String,
        encryption_key: Option<SecureKey>,
    ) -> Self {
        Self {
            write_half,
            target_ip,
            target_port,
            session_id,
            pair_code,
            encryption_key,
        }
    }

    /// 发送数据
    pub async fn send(
        &self,
        data: &[u8],
        transfer_id: &str,
        event_tx: tokio::sync::mpsc::UnboundedSender<P2PEvent>,
        session_id: &str,
    ) -> Result<(), P2PError> {
        let total_size = data.len() as u64;

        // 解析账单数量
        let bill_count = serde_json::from_slice::<serde_json::Value>(data)
            .ok()
            .and_then(|v| v.get("bills").and_then(|b| b.as_array()).map(|a| a.len()))
            .unwrap_or(0);

        // 发送 TransferOffer
        let offer = TransferOffer {
            transfer_id: transfer_id.to_string(),
            description: "Bill data transfer".to_string(),
            total_size,
            bill_count,
        };
        let offer_frame =
            encode_frame_maybe_encrypted(MSG_TYPE_TRANSFER_OFFER, &offer, self.encryption_key.as_ref())?;
        {
            let mut wh = self.write_half.lock().await;
            wh.write_all(&offer_frame).await?;
            wh.flush().await?;
        }

        // 等待 TransferAccept 通过 read_loop_inner 的 oneshot 通知
        let (accept_tx, accept_rx) = oneshot::channel();
        register_accept_waiter(transfer_id.to_string(), accept_tx).await;

        match accept_rx.await {
            Ok(Ok(())) => {
                // Transfer accepted
            }
            Ok(Err(e)) => {
                // Transfer rejected
                let error = TransferError {
                    session_id: session_id.to_string(),
                    direction: "send".to_string(),
                    error: format!("Transfer rejected: {}", e),
                };
                let _ = event_tx.send(P2PEvent::TransferError(error));
                return Err(e);
            }
            Err(_) => {
                // oneshot dropped (read loop ended)
                return Err(P2PError::TransferFailed(
                    "Connection closed while waiting for transfer accept".to_string(),
                ));
            }
        }

        let channel_key =
            open_transfer_channel(&self.target_ip, self.target_port, &self.session_id, transfer_id, &self.pair_code)
                .await?;

        // 分块发送数据
        let target = format!("{}:{}", self.target_ip, self.target_port);
        let mut stream = TcpStream::connect(&target).await.map_err(|e| {
            P2PError::ConnectionFailed(format!("Failed to open transfer channel to {}: {}", target, e))
        })?;
        let open = TransferChannelOpen {
            session_id: self.session_id.clone(),
            transfer_id: transfer_id.to_string(),
            pair_code: self.pair_code.clone(),
            salt: channel_key.1.clone(),
        };
        let open_frame = encode_transfer_channel_open(&open)?;
        stream.write_all(&open_frame).await?;
        stream.flush().await?;

        let ready_frame = ProtocolFrame::read_from_stream(&mut stream).await?;
        let ready_payload = decrypt_frame_payload(
            &ready_frame,
            Some(&channel_key.0),
        )?;
        let ready_frame = ProtocolFrame {
            msg_type: ready_frame.msg_type,
            payload: ready_payload,
        };
        if ready_frame.msg_type != MSG_TYPE_TRANSFER_CHANNEL_READY {
            return Err(P2PError::TransferFailed(format!(
                "Unexpected transfer channel response: {:#x}",
                ready_frame.msg_type
            )));
        }
        let ready: TransferChannelReady = decode_json(&ready_frame)?;
        if ready.transfer_id != transfer_id {
            return Err(P2PError::TransferFailed("Transfer channel ready mismatch".to_string()));
        }

        let mut offset = 0;
        let mut sequence: u32 = 0;
        let mut bytes_sent: u64 = 0;

        while offset < data.len() {
            let end = std::cmp::min(offset + CHUNK_SIZE, data.len());
            let chunk = &data[offset..end];

            let transfer_data = TransferData {
                transfer_id: transfer_id.to_string(),
                sequence,
                data: chunk.to_vec(),
            };

            let data_frame =
                encode_frame_maybe_encrypted(MSG_TYPE_TRANSFER_DATA, &transfer_data, Some(&channel_key.0))?;
            stream.write_all(&data_frame).await?;
            stream.flush().await?;

            bytes_sent += chunk.len() as u64;
            sequence += 1;
            offset = end;

            // 计算进度百分比
            let percentage = if total_size > 0 {
                (bytes_sent as f64 / total_size as f64) * 100.0
            } else {
                100.0
            };

            // 发送进度事件
            let progress = TransferProgress {
                session_id: session_id.to_string(),
                transfer_id: transfer_id.to_string(),
                bytes_transferred: bytes_sent,
                total_size,
                percentage,
            };
            let _ = event_tx.send(P2PEvent::TransferProgress(progress));
        }

        // 发送 TransferEnd
        let checksum = compute_checksum(data);
        let end_msg = TransferEnd {
            transfer_id: transfer_id.to_string(),
            checksum: checksum.clone(),
        };
        let end_frame =
            encode_frame_maybe_encrypted(MSG_TYPE_TRANSFER_END, &end_msg, Some(&channel_key.0))?;
        stream.write_all(&end_frame).await?;
        stream.flush().await?;

        let result_frame = ProtocolFrame::read_from_stream(&mut stream).await?;
        let result_payload = decrypt_frame_payload(&result_frame, Some(&channel_key.0))?;
        let result_frame = ProtocolFrame {
            msg_type: result_frame.msg_type,
            payload: result_payload,
        };
        if result_frame.msg_type != MSG_TYPE_TRANSFER_CHANNEL_RESULT {
            return Err(P2PError::TransferFailed(format!(
                "Unexpected transfer result type: {:#x}",
                result_frame.msg_type
            )));
        }
        let result: TransferChannelResult = decode_json(&result_frame)?;
        if !result.success {
            let error = TransferError {
                session_id: session_id.to_string(),
                direction: "send".to_string(),
                error: if is_blank(&result.reason) {
                    "Transfer failed on receiver".to_string()
                } else {
                    result.reason
                },
            };
            let _ = event_tx.send(P2PEvent::TransferError(error));
            return Err(P2PError::TransferFailed("Receiver reported transfer failure".to_string()));
        }

        tracing::info!(
            "[P2P] Transfer complete: {} bytes, {} chunks, checksum={}",
            bytes_sent,
            sequence,
            checksum
        );

        // 发送完成事件
        let complete = TransferComplete {
            session_id: session_id.to_string(),
            direction: "send".to_string(),
            bill_count,
            bytes_transferred: bytes_sent,
        };
        let _ = event_tx.send(P2PEvent::TransferComplete(complete));

        Ok(())
    }
}

pub async fn register_pending_incoming_transfer(pending: PendingIncomingTransfer) {
    let transfers = get_pending_incoming_transfers();
    let mut map = transfers.lock().await;
    map.insert(
        format!("{}:{}", pending.session_id, pending.transfer_id),
        pending,
    );
}

pub async fn take_pending_incoming_transfer(
    session_id: &str,
    transfer_id: &str,
) -> Option<PendingIncomingTransfer> {
    let transfers = get_pending_incoming_transfers();
    let mut map = transfers.lock().await;
    map.remove(&format!("{}:{}", session_id, transfer_id))
}

fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

async fn open_transfer_channel(
    _target_ip: &str,
    _target_port: u16,
    _session_id: &str,
    _transfer_id: &str,
    pair_code: &str,
) -> Result<(SecureKey, Vec<u8>), P2PError> {
    let salt = crypto::generate_salt().to_vec();
    let key_bytes = crypto::derive_key(pair_code, &salt);
    let key = SecureKey::new(key_bytes);
    Ok((key, salt))
}

/// 计算数据的简单校验和（FNV-1a hash 的十六进制表示）
pub fn compute_checksum(data: &[u8]) -> String {
    use std::fmt::Write;
    // FNV-1a hash 作为校验和
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let mut result = String::with_capacity(16);
    write!(result, "{:016x}", hash).unwrap();
    result
}

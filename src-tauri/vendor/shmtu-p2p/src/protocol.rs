use std::fmt;

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::crypto::SecureKey;

/// 默认 P2P 端口
pub const P2P_DEFAULT_PORT: u16 = 19827;

/// 协议魔数
pub const PROTOCOL_MAGIC: &[u8; 4] = b"SHTP";

/// 协议版本
pub const PROTOCOL_VERSION: u32 = 1;

/// 消息类型标记
pub const MSG_TYPE_PAIR_REQUEST: u8 = 0x01;
pub const MSG_TYPE_PAIR_ACCEPT: u8 = 0x02;
pub const MSG_TYPE_PAIR_REJECT: u8 = 0x03;
pub const MSG_TYPE_PING: u8 = 0x04;
pub const MSG_TYPE_PONG: u8 = 0x05;
pub const MSG_TYPE_ENCRYPTION_NEGOTIATE: u8 = 0x06;
pub const MSG_TYPE_ENCRYPTION_CONFIRM: u8 = 0x07;
pub const MSG_TYPE_TRANSFER_OFFER: u8 = 0x10;
pub const MSG_TYPE_TRANSFER_ACCEPT: u8 = 0x11;
pub const MSG_TYPE_TRANSFER_REJECT: u8 = 0x12;
pub const MSG_TYPE_TRANSFER_DATA: u8 = 0x13;
pub const MSG_TYPE_TRANSFER_END: u8 = 0x14;
pub const MSG_TYPE_TRANSFER_CHANNEL_OPEN: u8 = 0x15;
pub const MSG_TYPE_TRANSFER_CHANNEL_READY: u8 = 0x16;
pub const MSG_TYPE_TRANSFER_CHANNEL_RESULT: u8 = 0x17;
pub const MSG_TYPE_DISCONNECT: u8 = 0xFF;

/// 配对码（6 位大写字母）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairCode(String);

impl PairCode {
    /// 生成随机 6 位大写字母配对码
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let code: String = (0..6)
            .map(|_| {
                let idx = rng.gen_range(0..26);
                (b'A' + idx) as char
            })
            .collect();
        PairCode(code)
    }

    /// 从字符串创建配对码
    pub fn from_str_unchecked(s: &str) -> Self {
        PairCode(s.to_uppercase())
    }

    /// 获取配对码字符串
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 验证配对码格式（6 位大写字母）
    pub fn is_valid(&self) -> bool {
        self.0.len() == 6 && self.0.chars().all(|c| c.is_ascii_uppercase())
    }
}

impl fmt::Display for PairCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 配对请求消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairRequest {
    pub pair_code: String,
    pub device_name: String,
    #[serde(default)]
    pub listen_port: Option<u16>,
    #[serde(default)]
    pub listen_ips: Vec<String>,
}

/// 配对接受消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairAccept {
    pub device_name: String,
    pub session_id: String,
}

/// 配对拒绝消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairReject {
    pub reason: String,
}

/// 加密协商消息
///
/// 配对成功后由客户端发送，携带加密参数和盐值，
/// 双方基于配对码 + 盐值派生 AES-256 密钥。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionNegotiate {
    /// 加密方法，固定 "aes-256-gcm"
    pub method: String,
    /// 16 字节随机盐值
    pub salt: Vec<u8>,
    /// PBKDF2 迭代次数（600000）
    pub iterations: u32,
    /// 8 字节随机值，用于双方确认密钥一致
    pub client_nonce: Vec<u8>,
}

/// 加密确认消息
///
/// 服务端收到 EncryptionNegotiate 后派生密钥，
/// 计算验证 token 并返回此消息，客户端验证 token 确认双方密钥一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfirm {
    /// HMAC-SHA256(salt || client_nonce || "p2p-verify", derived_key) 的前 16 字节
    pub verification: Vec<u8>,
}

/// 传输提议消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOffer {
    pub transfer_id: String,
    pub description: String,
    pub total_size: u64,
    pub bill_count: usize,
}

/// 传输接受消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferAccept {
    pub transfer_id: String,
}

/// 传输拒绝消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferReject {
    pub transfer_id: String,
    pub reason: String,
}

/// 传输数据块消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferData {
    pub transfer_id: String,
    pub sequence: u32,
    pub data: Vec<u8>,
}

/// 传输结束消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferEnd {
    pub transfer_id: String,
    pub checksum: String,
}

/// 独立传输通道打开请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferChannelOpen {
    pub session_id: String,
    pub transfer_id: String,
    pub pair_code: String,
    pub salt: Vec<u8>,
}

/// 独立传输通道已就绪
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferChannelReady {
    pub transfer_id: String,
}

/// 独立传输通道结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferChannelResult {
    pub transfer_id: String,
    pub success: bool,
    pub reason: String,
}

/// 断开连接消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disconnect {
    pub reason: String,
}

/// 协议帧：4字节长度前缀 + 魔数 + 版本 + 消息类型 + 载荷
///
/// 帧格式:
///   [4 bytes: payload length (big-endian u32)]
///   [4 bytes: magic "SHTP"]
///   [4 bytes: protocol version (big-endian u32)]
///   [1 byte:  message type]
///   [N bytes: payload (明文或加密后的 JSON)]
///
/// 加密模式下 payload 部分加密，消息类型字段不加密（用于路由）。
pub struct ProtocolFrame {
    pub msg_type: u8,
    pub payload: Vec<u8>,
}

impl ProtocolFrame {
    /// 将消息编码为带长度前缀的帧
    pub fn encode(msg_type: u8, data: &[u8]) -> Vec<u8> {
        // 魔数(4) + 版本(4) + 类型(1) + 载荷
        let body_len = PROTOCOL_MAGIC.len() + 4 + 1 + data.len();
        let mut buf = Vec::with_capacity(4 + body_len);

        // 长度前缀 (不包含自身4字节)
        buf.extend_from_slice(&(body_len as u32).to_be_bytes());
        // 魔数
        buf.extend_from_slice(PROTOCOL_MAGIC);
        // 版本
        buf.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
        // 消息类型
        buf.push(msg_type);
        // 载荷
        buf.extend_from_slice(data);

        buf
    }

    /// 从 TCP 流读取一帧
    pub async fn read_from_stream<R: tokio::io::AsyncReadExt + Unpin>(
        stream: &mut R,
    ) -> Result<Self, ProtocolError> {
        // 读取长度前缀
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                ProtocolError::ConnectionClosed
            } else {
                ProtocolError::Io(e)
            }
        })?;
        let body_len = u32::from_be_bytes(len_buf) as usize;

        if body_len > 10 * 1024 * 1024 {
            return Err(ProtocolError::FrameTooLarge(body_len));
        }

        // 读取帧体
        let mut body = vec![0u8; body_len];
        stream.read_exact(&mut body).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                ProtocolError::ConnectionClosed
            } else {
                ProtocolError::Io(e)
            }
        })?;

        // 验证魔数
        if body.len() < 9 {
            return Err(ProtocolError::InvalidFrame("frame too short".to_string()));
        }
        if &body[0..4] != PROTOCOL_MAGIC {
            return Err(ProtocolError::InvalidMagic);
        }

        // 读取版本
        let version = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
        if version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(version));
        }

        // 读取消息类型
        let msg_type = body[8];

        // 载荷
        let payload = body[9..].to_vec();

        Ok(ProtocolFrame { msg_type, payload })
    }

    /// 编码 JSON 消息
    pub fn encode_json<T: Serialize>(msg_type: u8, msg: &T) -> Result<Vec<u8>, ProtocolError> {
        let json = serde_json::to_vec(msg)?;
        Ok(Self::encode(msg_type, &json))
    }
}

/// 协议错误
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("连接已关闭")]
    ConnectionClosed,

    #[error("帧过大: {0} bytes")]
    FrameTooLarge(usize),

    #[error("无效帧: {0}")]
    InvalidFrame(String),

    #[error("无效魔数")]
    InvalidMagic,

    #[error("不支持的协议版本: {0}")]
    UnsupportedVersion(u32),

    #[error("未知消息类型: {0:#x}")]
    UnknownMessageType(u8),

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// 判断消息类型是否应该加密
///
/// PING/PONG 消息 payload 为空，不需要加密。
/// EncryptionNegotiate/EncryptionConfirm 本身也不加密（协商阶段）。
/// DISCONNECT 消息在加密后发送。
pub fn should_encrypt_msg(msg_type: u8) -> bool {
    matches!(
        msg_type,
        MSG_TYPE_TRANSFER_OFFER
            | MSG_TYPE_TRANSFER_ACCEPT
            | MSG_TYPE_TRANSFER_REJECT
            | MSG_TYPE_TRANSFER_DATA
            | MSG_TYPE_TRANSFER_END
            | MSG_TYPE_TRANSFER_CHANNEL_READY
            | MSG_TYPE_TRANSFER_CHANNEL_RESULT
            | MSG_TYPE_DISCONNECT
    )
}

/// 加密 payload 并构建帧
///
/// 仅对需要加密的消息类型加密，否则直接构建帧。
/// 消除 client.rs 和 transfer.rs 中的重复加密编码逻辑。
pub fn encode_frame_maybe_encrypted<T: Serialize>(
    msg_type: u8,
    msg: &T,
    encryption_key: Option<&SecureKey>,
) -> Result<Vec<u8>, ProtocolError> {
    let json = serde_json::to_vec(msg)?;
    let payload = if should_encrypt_msg(msg_type) {
        if let Some(key) = encryption_key {
            let encrypted = crate::crypto::encrypt(key.as_bytes(), &json)
                .map_err(|e| ProtocolError::InvalidFrame(format!("Encryption failed: {}", e)))?;
            encrypted
        } else {
            json
        }
    } else {
        json
    };
    Ok(ProtocolFrame::encode(msg_type, &payload))
}

/// 解密帧 payload（如果有加密密钥且消息类型需要解密）
///
/// 统一 read_loop_inner 中的帧解密逻辑。
pub fn decrypt_frame_payload(
    frame: &ProtocolFrame,
    encryption_key: Option<&SecureKey>,
) -> Result<Vec<u8>, ProtocolError> {
    if should_encrypt_msg(frame.msg_type) {
        if let Some(key) = encryption_key {
            crate::crypto::decrypt(key.as_bytes(), &frame.payload)
                .map_err(|e| ProtocolError::InvalidFrame(format!("Decryption failed: {}", e)))
        } else {
            Ok(frame.payload.clone())
        }
    } else {
        Ok(frame.payload.clone())
    }
}

/// 将配对请求编码为帧
pub fn encode_pair_request(req: &PairRequest) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_PAIR_REQUEST, req)
}

/// 将配对接受编码为帧
pub fn encode_pair_accept(accept: &PairAccept) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_PAIR_ACCEPT, accept)
}

/// 将配对拒绝编码为帧
pub fn encode_pair_reject(reject: &PairReject) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_PAIR_REJECT, reject)
}

/// 将加密协商编码为帧
pub fn encode_encryption_negotiate(neg: &EncryptionNegotiate) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_ENCRYPTION_NEGOTIATE, neg)
}

/// 将加密确认编码为帧
pub fn encode_encryption_confirm(confirm: &EncryptionConfirm) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_ENCRYPTION_CONFIRM, confirm)
}

/// 将传输提议编码为帧
pub fn encode_transfer_offer(offer: &TransferOffer) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_TRANSFER_OFFER, offer)
}

/// 将传输接受编码为帧
pub fn encode_transfer_accept(accept: &TransferAccept) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_TRANSFER_ACCEPT, accept)
}

/// 将传输拒绝编码为帧
pub fn encode_transfer_reject(reject: &TransferReject) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_TRANSFER_REJECT, reject)
}

/// 将传输数据编码为帧
pub fn encode_transfer_data(data: &TransferData) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_TRANSFER_DATA, data)
}

/// 将传输结束编码为帧
pub fn encode_transfer_end(end: &TransferEnd) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_TRANSFER_END, end)
}

/// 将独立传输通道打开请求编码为帧
pub fn encode_transfer_channel_open(
    open: &TransferChannelOpen,
) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_TRANSFER_CHANNEL_OPEN, open)
}

/// 将独立传输通道就绪消息编码为帧
pub fn encode_transfer_channel_ready(
    ready: &TransferChannelReady,
) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_TRANSFER_CHANNEL_READY, ready)
}

/// 将独立传输通道结果编码为帧
pub fn encode_transfer_channel_result(
    result: &TransferChannelResult,
) -> Result<Vec<u8>, ProtocolError> {
    ProtocolFrame::encode_json(MSG_TYPE_TRANSFER_CHANNEL_RESULT, result)
}

/// 将断开连接编码为帧
pub fn encode_disconnect(reason: &str) -> Result<Vec<u8>, ProtocolError> {
    let msg = Disconnect {
        reason: reason.to_string(),
    };
    ProtocolFrame::encode_json(MSG_TYPE_DISCONNECT, &msg)
}

/// 将 PING 消息编码为帧
pub fn encode_ping() -> Result<Vec<u8>, ProtocolError> {
    Ok(ProtocolFrame::encode(MSG_TYPE_PING, &[]))
}

/// 将 PONG 消息编码为帧
pub fn encode_pong() -> Result<Vec<u8>, ProtocolError> {
    Ok(ProtocolFrame::encode(MSG_TYPE_PONG, &[]))
}

/// 解码帧载荷为 JSON 类型
pub fn decode_json<T: for<'de> Deserialize<'de>>(
    frame: &ProtocolFrame,
) -> Result<T, ProtocolError> {
    serde_json::from_slice(&frame.payload).map_err(ProtocolError::from)
}

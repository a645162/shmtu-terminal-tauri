//! P2P 传输加密模块
//!
//! 使用基于配对码的密钥派生方案：
//! 配对成功后，双方用 PBKDF2 从配对码派生 AES-256 密钥，
//! 之后所有帧的 payload 用 AES-256-GCM 加密。

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use zeroize::Zeroize;

/// PBKDF2 迭代次数
pub const PBKDF2_ITERATIONS: u32 = 600_000;

/// Salt 长度（字节）
pub const SALT_LEN: usize = 16;

/// AES-GCM Nonce 长度（字节）
pub const NONCE_LEN: usize = 12;

/// AES-256 密钥长度（字节）
pub const KEY_LEN: usize = 32;

/// 验证 token 长度（字节）
pub const VERIFICATION_LEN: usize = 16;

type HmacSha256 = Hmac<Sha256>;

/// 安全密钥包装，drop 时自动清零
///
/// 包装 AES-256 密钥材料，确保密钥在离开作用域时被安全擦除，
/// 防止密钥残留在内存中。同时阻止 Debug trait 泄露密钥内容。
#[derive(Clone)]
pub struct SecureKey([u8; KEY_LEN]);

impl SecureKey {
    /// 从原始字节创建 SecureKey
    pub fn new(key: [u8; KEY_LEN]) -> Self {
        Self(key)
    }

    /// 获取密钥的字节引用
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl Drop for SecureKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl std::fmt::Debug for SecureKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecureKey([REDACTED; 32])")
    }
}

/// 从配对码派生 AES-256 密钥
///
/// 使用 PBKDF2-HMAC-SHA256 从配对码和盐值派生 32 字节密钥。
pub fn derive_key(pair_code: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(pair_code.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

/// AES-256-GCM 加密
///
/// 返回格式: `nonce(12) || ciphertext || tag(16)`
pub fn encrypt(key: &[u8; KEY_LEN], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength)?;
    let nonce_bytes: [u8; NONCE_LEN] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)?;
    // nonce(12) + ciphertext + tag(16)
    let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// AES-256-GCM 解密
///
/// 输入格式: `nonce(12) || ciphertext || tag(16)`
pub fn decrypt(key: &[u8; KEY_LEN], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < NONCE_LEN + 16 {
        // 至少需要 nonce + tag
        return Err(CryptoError::DataTooShort);
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength)?;
    let nonce = Nonce::from_slice(&data[..NONCE_LEN]);
    cipher
        .decrypt(nonce, &data[NONCE_LEN..])
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// 生成验证 token
///
/// 计算 `HMAC-SHA256(salt || client_nonce || "p2p-verify", derived_key)` 的前 16 字节，
/// 用于双方确认密钥一致。client_nonce 参与计算可防止预计算攻击。
pub fn generate_verification(
    salt: &[u8],
    client_nonce: &[u8],
    key: &[u8; KEY_LEN],
) -> [u8; VERIFICATION_LEN] {
    let mut mac =
        <HmacSha256 as hmac::Mac>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(salt);
    mac.update(client_nonce);
    mac.update(b"p2p-verify");
    let result = mac.finalize().into_bytes();
    let mut verification = [0u8; VERIFICATION_LEN];
    verification.copy_from_slice(&result[..VERIFICATION_LEN]);
    verification
}

/// 生成随机 salt
pub fn generate_salt() -> [u8; SALT_LEN] {
    rand::random()
}

/// 加密错误类型
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Invalid key length")]
    InvalidKeyLength,

    #[error("Encrypted data too short")]
    DataTooShort,

    #[error("Encryption failed")]
    EncryptionFailed,

    #[error("Decryption failed")]
    DecryptionFailed,
}

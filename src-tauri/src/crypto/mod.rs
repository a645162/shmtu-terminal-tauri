use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

/// 加密服务，提供密码、Cookie 等敏感数据的加密/解密功能
pub struct CryptoService {
    key: [u8; 32],
}

impl CryptoService {
    /// 使用原始密钥字节创建加密服务
    pub fn new(key: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(key);
        let hash = hasher.finalize();
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&hash);
        Self { key: key_bytes }
    }

    /// 使用密码字符串创建加密服务
    pub fn from_password(password: &str) -> Self {
        Self::new(password.as_bytes())
    }

    /// 使用设备唯一标识派生密钥创建加密服务
    pub fn from_device_id(device_id: &str) -> Self {
        Self::new(device_id.as_bytes())
    }

    /// 生成随机密钥
    pub fn generate_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// 加密数据，返回 Base64 编码的密文（含 nonce 前缀）
    pub fn encrypt(&self, plaintext: &[u8]) -> AppResult<String> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| AppError::Crypto(format!("创建加密器失败: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| AppError::Crypto(format!("加密失败: {}", e)))?;

        // nonce(12 bytes) + ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(&result))
    }

    /// 解密 Base64 编码的密文
    pub fn decrypt(&self, encoded: &str) -> AppResult<Vec<u8>> {
        let data = BASE64
            .decode(encoded)
            .map_err(|e| AppError::Crypto(format!("Base64解码失败: {}", e)))?;

        if data.len() < 12 {
            return Err(AppError::Crypto("密文数据过短".to_string()));
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| AppError::Crypto(format!("创建解密器失败: {}", e)))?;

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::Crypto(format!("解密失败: {}", e)))
    }

    /// 加密字符串
    pub fn encrypt_string(&self, plaintext: &str) -> AppResult<String> {
        self.encrypt(plaintext.as_bytes())
    }

    /// 解密为字符串
    pub fn decrypt_string(&self, encoded: &str) -> AppResult<String> {
        let bytes = self.decrypt(encoded)?;
        String::from_utf8(bytes).map_err(|e| AppError::Crypto(format!("UTF-8解码失败: {}", e)))
    }

    /// 计算密码的 SHA-256 哈希（用于启动保护密码校验）
    pub fn hash_password(password: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let hash = hasher.finalize();
        hex::encode(hash)
    }

    /// 验证密码是否匹配哈希
    pub fn verify_password(password: &str, hash: &str) -> bool {
        let computed = Self::hash_password(password);
        // 常量时间比较，防止时序攻击
        // Use subtle::ConstantTimeEq for constant-time comparison
        use subtle::ConstantTimeEq;
        computed.as_bytes().ct_eq(hash.as_bytes()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let crypto = CryptoService::from_password("test_password");
        let plaintext = "Hello, 世界!";
        let encrypted = crypto.encrypt_string(plaintext).unwrap();
        let decrypted = crypto.decrypt_string(&encrypted).unwrap();
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_password_hash() {
        let hash = CryptoService::hash_password("mypassword");
        assert!(CryptoService::verify_password("mypassword", &hash));
        assert!(!CryptoService::verify_password("wrongpassword", &hash));
    }

    #[test]
    fn test_different_keys_produce_different_ciphertext() {
        let crypto1 = CryptoService::from_password("key1");
        let crypto2 = CryptoService::from_password("key2");
        let encrypted1 = crypto1.encrypt_string("test").unwrap();
        let encrypted2 = crypto2.encrypt_string("test").unwrap();
        assert_ne!(encrypted1, encrypted2);
    }
}

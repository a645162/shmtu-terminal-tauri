use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;

use super::webdav::{BackupMeta, UploadResult, WebDavProvider, WebDavConfig};

/// 云备份管理器
pub struct CloudBackupManager {
    pub webdav: WebDavProvider,
}

impl CloudBackupManager {
    pub fn new() -> Self {
        Self {
            webdav: WebDavProvider::new(),
        }
    }

    pub fn configure_webdav(&mut self, config: WebDavConfig) {
        self.webdav.configure(config);
    }

    pub async fn test_connection(&self) -> Result<bool> {
        self.webdav.test_connection().await
    }

    pub async fn test_write_read(&self) -> Result<String> {
        self.webdav.test_write_read().await
    }

    /// 构建归档并上传
    pub async fn backup_now(
        &self,
        data_dir: &std::path::Path,
        identity_id: i64,
        password: Option<&str>,
        backup_root: &str,
    ) -> Result<UploadResult> {
        let archive_bytes = build_transfer_archive(data_dir, identity_id)?;
        let (final_bytes, encrypted) = if let Some(pwd) = password {
            (encrypt_archive(&archive_bytes, pwd)?, true)
        } else {
            (archive_bytes, false)
        };

        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let ext = if encrypted { "zip.enc" } else { "zip" };
        let file_name = format!("shmtu-backup-{}.{}", timestamp, ext);
        let remote_path = format!("{}/{}", backup_root.trim_start_matches('/'), file_name);

        let result = self.webdav.upload(&remote_path, &final_bytes).await?;
        tracing::info!("[CloudBackup] backup uploaded: {} bytes, encrypted={}", result.bytes, encrypted);
        Ok(result)
    }

    /// 下载并恢复
    pub async fn restore_backup(
        &self,
        remote_path: &str,
        password: Option<&str>,
        data_dir: &std::path::Path,
    ) -> Result<RestoreReport> {
        let encrypted = remote_path.ends_with(".enc");
        let downloaded = self.webdav.download(remote_path).await?;
        let plain_bytes: Vec<u8> = if encrypted {
            let pwd = password.ok_or_else(|| anyhow!("备份已加密，请提供密码"))?;
            decrypt_archive(&downloaded, pwd)?
        } else {
            downloaded
        };
        let report = import_transfer_archive(data_dir, &plain_bytes)?;
        tracing::info!("[CloudBackup] restore: {} identities, {} accounts, {} bills",
            report.identity_count, report.account_count, report.bill_count);
        Ok(report)
    }

    pub async fn list_remote_backups(&self, backup_root: &str) -> Result<Vec<BackupMeta>> {
        self.webdav.list(backup_root).await
    }

    pub async fn delete_remote_backup(&self, remote_path: &str) -> Result<bool> {
        self.webdav.delete(remote_path).await
    }

    pub async fn prune_old_backups(&self, backup_root: &str, max_keep: usize) -> Result<usize> {
        let mut list = self.list_remote_backups(backup_root).await?;
        if list.len() <= max_keep { return Ok(0); }
        list.sort_by(|a, b| b.name.cmp(&a.name));
        let to_delete = &list[max_keep..];
        let mut deleted = 0;
        for meta in to_delete {
            if self.webdav.delete(&meta.remote_path).await.unwrap_or(false) { deleted += 1; }
        }
        tracing::info!("[CloudBackup] pruned {} old backups, keeping {}", deleted, max_keep);
        Ok(deleted)
    }
}

// ============= 归档格式（与 Android schema_version=2 兼容）=============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferArchive {
    pub schema_version: u32,
    pub export_time: i64,
    pub format: String,
    pub source_platform: String,
    pub identities: Vec<IdentityBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityBundle {
    pub identity: IdentityInfo,
    pub accounts: Vec<AccountBundle>,
    pub bills: Vec<BillInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityInfo {
    pub name: String,
    pub enable: bool,
    pub birthday: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBundle {
    pub entity: AccountInfo,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub account_name: String,
    pub account_id: String,
    pub enable: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillInfo {
    pub date_time_formatted: Option<String>,
    pub item_type: Option<String>,
    pub number: Option<String>,
    pub target_user: Option<String>,
    pub money_str: Option<String>,
    pub money: Option<f64>,
    pub method: Option<String>,
    pub status_str: Option<String>,
    pub is_combined: bool,
    pub source_account_id: Option<String>,
    pub position: Option<String>,
    pub category: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    pub identity_count: usize,
    pub account_count: usize,
    pub bill_count: usize,
}

fn build_transfer_archive(_data_dir: &std::path::Path, _identity_id: i64) -> Result<Vec<u8>> {
    let archive = TransferArchive {
        schema_version: 2,
        export_time: chrono::Utc::now().timestamp_millis(),
        format: "shmtu-transfer-archive".to_string(),
        source_platform: "tauri".to_string(),
        identities: vec![],
    };
    let json_str = serde_json::to_string_pretty(&archive)?;
    let zip_out = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(zip_out);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options)?;
    zip.write_all(json_str.as_bytes())?;
    Ok(zip.finish()?.into_inner())
}

fn import_transfer_archive(_data_dir: &std::path::Path, _data: &[u8]) -> Result<RestoreReport> {
    Ok(RestoreReport { identity_count: 0, account_count: 0, bill_count: 0 })
}

// ============= 加密（与 Android PBKDF2+AES-256-GCM 兼容）=============

fn encrypt_archive(data: &[u8], password: &str) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit, OsRng};
    use aes_gcm::{Aes256Gcm, Nonce};
    use rand::RngCore;

    let mut salt = [0u8; 16]; OsRng.fill_bytes(&mut salt);
    let mut iv = [0u8; 12]; OsRng.fill_bytes(&mut iv);

    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow!("加密器创建失败: {}", e))?;
    let ciphertext = cipher.encrypt(Nonce::from_slice(&iv), data)
        .map_err(|e| anyhow!("加密失败: {}", e))?;

    let mut out = Vec::with_capacity(16 + 12 + ciphertext.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&iv);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn decrypt_archive(data: &[u8], password: &str) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    if data.len() < 16 + 12 + 16 { return Err(anyhow!("加密数据过短")); }
    let (salt, rest) = data.split_at(16);
    let (iv, ciphertext) = rest.split_at(12);

    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow!("解密器创建失败: {}", e))?;
    cipher.decrypt(Nonce::from_slice(iv), ciphertext)
        .map_err(|e| anyhow!("解密失败: {}", e))
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let password_bytes = password.as_bytes();
    let mut key = [0u8; 32];

    for block_index in 1u32..=1u32 {
        let mut mac = HmacSha256::new_from_slice(password_bytes)
            .map_err(|e| anyhow!("HMAC init: {}", e))?;
        mac.update(salt);
        mac.update(&block_index.to_be_bytes());
        let mut u = mac.finalize().into_bytes();
        let mut result = u.clone();

        for _ in 1..100_000u32 {
            let mut mac2 = HmacSha256::new_from_slice(password_bytes)
                .map_err(|e| anyhow!("HMAC init: {}", e))?;
            mac2.update(&u);
            u = mac2.finalize().into_bytes();
            for (r, ub) in result.iter_mut().zip(u.iter()) { *r ^= ub; }
        }
        key.copy_from_slice(&result[..32]);
    }
    Ok(key)
}

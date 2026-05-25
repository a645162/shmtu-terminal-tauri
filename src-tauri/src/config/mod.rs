use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::AppResult;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub identity: IdentityConfig,
    #[serde(default)]
    pub captcha: CaptchaConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub data: DataConfig,
    #[serde(default)]
    pub classification: ClassificationConfig,
    #[serde(default)]
    pub update: UpdateConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub enable_startup_protection: bool,
    #[serde(default)]
    pub password_hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityConfig {
    #[serde(default)]
    pub remember_default: bool,
    #[serde(default)]
    pub default_identity_id: i64,
    #[serde(default)]
    pub last_identity_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptchaMode {
    Manual,
    RemoteOcr,
    LocalOnnx,
}

#[allow(clippy::derivable_impls)]
impl Default for CaptchaMode {
    fn default() -> Self {
        CaptchaMode::Manual
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CaptchaConfig {
    #[serde(default)]
    pub mode: CaptchaMode,
    #[serde(default)]
    pub remote_ocr_host: String,
    #[serde(default)]
    pub remote_ocr_port: u16,
    #[serde(default)]
    pub onnx_model_path: String,
    #[serde(default = "default_ocr_retry_count")]
    pub ocr_retry_count: usize,
}

fn default_ocr_retry_count() -> usize {
    3
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
    #[serde(default = "default_early_stop_threshold")]
    pub early_stop_threshold: u32,
    #[serde(default = "default_true")]
    pub auto_merge_after_sync: bool,
}

fn default_max_pages() -> u32 {
    100
}
fn default_early_stop_threshold() -> u32 {
    5
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataConfig {
    #[serde(default = "default_data_directory")]
    pub data_directory: String,
    #[serde(default = "default_snapshot_keep_count")]
    pub snapshot_keep_count: usize,
}

fn default_data_directory() -> String {
    "Data".to_string()
}
fn default_snapshot_keep_count() -> usize {
    10
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassificationConfig {
    #[serde(default)]
    pub rules_path: String,
    #[serde(default)]
    pub rules_update_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateConfig {
    #[serde(default = "default_true")]
    pub auto_check: bool,
    #[serde(default = "default_check_interval")]
    pub check_interval_hours: u64,
    #[serde(default)]
    pub last_check_time: String,
}

fn default_check_interval() -> u64 {
    24
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_theme() -> String {
    "light".to_string()
}
fn default_language() -> String {
    "zh-CN".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Session 续期检查间隔（分钟），默认 10 分钟
    #[serde(default = "default_session_refresh_interval")]
    pub refresh_interval_minutes: u64,
    /// 是否启用自动 session 续期
    #[serde(default = "default_true")]
    pub auto_refresh: bool,
}

fn default_session_refresh_interval() -> u64 {
    10
}

pub struct TomlConfig {
    config_dir: PathBuf,
    config: AppConfig,
}

impl TomlConfig {
    pub fn load(config_dir: impl AsRef<Path>) -> AppResult<Self> {
        let config_dir = config_dir.as_ref().to_path_buf();
        let config_path = config_dir.join("app_config.toml");
        let config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content)?
        } else {
            AppConfig::default()
        };
        Ok(Self { config_dir, config })
    }

    pub fn save(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        let config_path = self.config_dir.join("app_config.toml");
        let content = toml::to_string_pretty(&self.config)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn get(&self) -> &AppConfig {
        &self.config
    }

    pub fn get_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    pub fn update(&mut self, config: AppConfig) -> AppResult<()> {
        self.config = config;
        self.save()
    }

    pub fn reset_to_default(&mut self) -> AppResult<()> {
        self.config = AppConfig::default();
        self.save()
    }

    pub fn verify_startup_password(&self, password: &str) -> bool {
        if !self.config.security.enable_startup_protection {
            return true;
        }
        crate::crypto::CryptoService::verify_password(password, &self.config.security.password_hash)
    }

    pub fn set_startup_password(&mut self, password: &str) -> AppResult<()> {
        self.config.security.password_hash = crate::crypto::CryptoService::hash_password(password);
        self.config.security.enable_startup_protection = true;
        self.save()
    }

    pub fn disable_startup_protection(&mut self) -> AppResult<()> {
        self.config.security.enable_startup_protection = false;
        self.config.security.password_hash = String::new();
        self.save()
    }

    pub fn set_default_identity(&mut self, identity_id: i64) -> AppResult<()> {
        self.config.identity.default_identity_id = identity_id;
        self.save()
    }

    pub fn clear_default_identity(&mut self) -> AppResult<()> {
        self.config.identity.default_identity_id = 0;
        self.save()
    }

    pub fn set_last_identity(&mut self, identity_id: i64) -> AppResult<()> {
        self.config.identity.last_identity_id = identity_id;
        self.save()
    }

    pub fn classification_rules_path(&self) -> PathBuf {
        if self.config.classification.rules_path.is_empty() {
            self.config_dir.join("classification_rules.toml")
        } else {
            PathBuf::from(&self.config.classification.rules_path)
        }
    }

    pub fn data_directory(&self) -> PathBuf {
        if self.config.data.data_directory.is_empty() {
            PathBuf::from("Data")
        } else {
            PathBuf::from(&self.config.data.data_directory)
        }
    }

    pub fn onnx_model_path(&self) -> PathBuf {
        if self.config.captcha.onnx_model_path.is_empty() {
            self.data_directory().join("models")
        } else {
            PathBuf::from(&self.config.captcha.onnx_model_path)
        }
    }
}

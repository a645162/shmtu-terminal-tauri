use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::AppResult;

/// 全局应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            security: SecurityConfig::default(),
            identity: IdentityConfig::default(),
            captcha: CaptchaConfig::default(),
            sync: SyncConfig::default(),
            data: DataConfig::default(),
            classification: ClassificationConfig::default(),
            update: UpdateConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub enable_startup_protection: bool,
    #[serde(default)]
    pub password_hash: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_startup_protection: false,
            password_hash: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityConfig {
    #[serde(default)]
    pub remember_default: bool,
    #[serde(default)]
    pub default_identity_id: i64,
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            remember_default: false,
            default_identity_id: 0,
        }
    }
}

/// 验证码识别模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptchaMode {
    Manual,
    RemoteOcr,
    LocalOnnx,
}

impl Default for CaptchaMode {
    fn default() -> Self {
        CaptchaMode::Manual
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for CaptchaConfig {
    fn default() -> Self {
        Self {
            mode: CaptchaMode::Manual,
            remote_ocr_host: String::new(),
            remote_ocr_port: 0,
            onnx_model_path: String::new(),
            ocr_retry_count: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            max_pages: 100,
            early_stop_threshold: 5,
            auto_merge_after_sync: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            data_directory: "Data".to_string(),
            snapshot_keep_count: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationConfig {
    #[serde(default)]
    pub rules_path: String,
    #[serde(default)]
    pub rules_update_url: String,
}

impl Default for ClassificationConfig {
    fn default() -> Self {
        Self {
            rules_path: String::new(),
            rules_update_url: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_check: true,
            check_interval_hours: 24,
            last_check_time: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "light".to_string(),
            language: "zh-CN".to_string(),
        }
    }
}

/// TOML 配置文件读写服务
pub struct TomlConfig {
    config_dir: PathBuf,
    config: AppConfig,
}

impl TomlConfig {
    /// 从指定目录加载配置
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

    /// 保存配置到文件
    pub fn save(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        let config_path = self.config_dir.join("app_config.toml");
        let content = toml::to_string_pretty(&self.config)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// 获取配置引用
    pub fn get(&self) -> &AppConfig {
        &self.config
    }

    /// 获取配置可变引用（修改后需调用 save 保存）
    pub fn get_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    /// 更新配置并保存
    pub fn update(&mut self, config: AppConfig) -> AppResult<()> {
        self.config = config;
        self.save()
    }

    /// 重置为默认配置
    pub fn reset_to_default(&mut self) -> AppResult<()> {
        self.config = AppConfig::default();
        self.save()
    }

    /// 验证启动密码
    pub fn verify_startup_password(&self, password: &str) -> bool {
        if !self.config.security.enable_startup_protection {
            return true;
        }
        crate::crypto::CryptoService::verify_password(
            password,
            &self.config.security.password_hash,
        )
    }

    /// 设置启动保护密码
    pub fn set_startup_password(&mut self, password: &str) -> AppResult<()> {
        self.config.security.password_hash = crate::crypto::CryptoService::hash_password(password);
        self.config.security.enable_startup_protection = true;
        self.save()
    }

    /// 关闭启动保护
    pub fn disable_startup_protection(&mut self) -> AppResult<()> {
        self.config.security.enable_startup_protection = false;
        self.config.security.password_hash = String::new();
        self.save()
    }

    /// 设置默认身份
    pub fn set_default_identity(&mut self, identity_id: i64) -> AppResult<()> {
        self.config.identity.remember_default = true;
        self.config.identity.default_identity_id = identity_id;
        self.save()
    }

    /// 清除默认身份
    pub fn clear_default_identity(&mut self) -> AppResult<()> {
        self.config.identity.remember_default = false;
        self.config.identity.default_identity_id = 0;
        self.save()
    }

    /// 获取分类规则文件路径（优先使用配置中的路径，否则使用默认路径）
    pub fn classification_rules_path(&self) -> PathBuf {
        if self.config.classification.rules_path.is_empty() {
            self.config_dir.join("classification_rules.toml")
        } else {
            PathBuf::from(&self.config.classification.rules_path)
        }
    }

    /// 获取数据目录路径
    pub fn data_directory(&self) -> PathBuf {
        if self.config.data.data_directory.is_empty() {
            PathBuf::from("Data")
        } else {
            PathBuf::from(&self.config.data.data_directory)
        }
    }

    /// 获取 ONNX 模型路径（优先使用配置中的路径，否则使用默认路径）
    pub fn onnx_model_path(&self) -> PathBuf {
        if self.config.captcha.onnx_model_path.is_empty() {
            self.data_directory().join("models")
        } else {
            PathBuf::from(&self.config.captcha.onnx_model_path)
        }
    }
}

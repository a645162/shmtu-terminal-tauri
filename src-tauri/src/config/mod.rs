use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::AppResult;

/// 应用全局配置
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
    #[serde(default)]
    pub p2p: shmtu_p2p::P2PConfig,
}

/// 安全配置（启动密码保护）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub enable_startup_protection: bool,
    #[serde(default)]
    pub password_hash: String,
}

/// 身份相关配置（默认身份、上次使用的身份）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityConfig {
    #[serde(default)]
    pub remember_default: bool,
    #[serde(default)]
    pub default_identity_id: i64,
    #[serde(default)]
    pub last_identity_id: i64,
}

/// 验证码识别模式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptchaMode {
    Manual,
    RemoteOcr,
    RemoteOcrHttp,
    LocalOnnx,
}

#[allow(clippy::derivable_impls)]
impl Default for CaptchaMode {
    fn default() -> Self {
        CaptchaMode::Manual
    }
}

/// 验证码配置（识别模式、远程 OCR 地址、ONNX 模型路径）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CaptchaConfig {
    #[serde(default)]
    pub mode: CaptchaMode,
    #[serde(default)]
    pub remote_ocr_host: String,
    #[serde(default)]
    pub remote_ocr_port: u16,
    #[serde(default = "default_remote_ocr_http_url")]
    pub remote_ocr_http_url: String,
    #[serde(default)]
    pub onnx_model_path: String,
    #[serde(default = "default_ocr_retry_count")]
    pub ocr_retry_count: usize,
}

fn default_remote_ocr_http_url() -> String {
    "http://127.0.0.1:5000".to_string()
}

fn default_ocr_retry_count() -> usize {
    5
}

/// 同步配置（最大页数、提前停止阈值、同步后自动合并）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
    #[serde(default = "default_early_stop_threshold")]
    pub early_stop_threshold: u32,
    #[serde(default = "default_true")]
    pub skip_graduated_accounts: bool,
    #[serde(default = "default_true")]
    pub auto_merge_after_sync: bool,
    #[serde(default)]
    pub auto_sync_enabled: bool,
    #[serde(default = "default_auto_sync_interval_minutes")]
    pub auto_sync_interval_minutes: u64,
    #[serde(default = "default_auto_sync_range")]
    pub auto_sync_range: SyncRangePresetConfig,
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
fn default_auto_sync_interval_minutes() -> u64 {
    60
}
fn default_auto_sync_range() -> SyncRangePresetConfig {
    SyncRangePresetConfig::Month
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRangePresetConfig {
    Week,
    HalfMonth,
    Month,
    HalfYear,
    Year,
    All,
}

impl Default for SyncRangePresetConfig {
    fn default() -> Self {
        Self::Month
    }
}

/// 数据目录配置
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

/// 分类规则配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassificationConfig {
    #[serde(default)]
    pub rules_path: String,
    #[serde(default)]
    pub rules_update_url: String,
}

/// 自动更新配置
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

/// UI 配置（主题、语言、小数位数、首页图表范围）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
    /// 统计数值保留小数位数，默认 2
    #[serde(default = "default_decimal_places")]
    pub decimal_places: u32,
    /// 首页趋势图表时间范围，默认 "week"
    #[serde(default = "default_home_trend_range")]
    pub home_trend_range: String,
    /// 首页分类图表时间范围，默认 "month"
    #[serde(default = "default_home_category_range")]
    pub home_category_range: String,
}

fn default_theme() -> String {
    "light".to_string()
}
fn default_language() -> String {
    "zh-CN".to_string()
}
fn default_decimal_places() -> u32 {
    2
}
fn default_home_trend_range() -> String {
    "week".to_string()
}
fn default_home_category_range() -> String {
    "month".to_string()
}

/// 会话续期配置
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

/// TOML 配置文件管理器
pub struct TomlConfig {
    config_dir: PathBuf,
    config: AppConfig,
}

impl TomlConfig {
    /// 从配置目录加载 TOML 配置文件。
    ///
    /// 若文件不存在则使用默认配置并输出警告日志。
    pub fn load(config_dir: impl AsRef<Path>) -> AppResult<Self> {
        let config_dir = config_dir.as_ref().to_path_buf();
        let config_path = config_dir.join("app_config.toml");
        let config = if config_path.exists() {
            tracing::info!("[Config] Loading config from {}", config_path.display());
            let content = std::fs::read_to_string(&config_path)?;
            match toml::from_str(&content) {
                Ok(c) => {
                    tracing::info!(
                        "[Config] Config loaded successfully from {}",
                        config_path.display()
                    );
                    c
                }
                Err(e) => {
                    tracing::warn!(
                        "[Config] Failed to parse config file {}, using defaults: {}",
                        config_path.display(),
                        e
                    );
                    AppConfig::default()
                }
            }
        } else {
            tracing::warn!(
                "[Config] Config file not found at {}, using defaults",
                config_path.display()
            );
            AppConfig::default()
        };
        Ok(Self { config_dir, config })
    }

    /// 将当前配置写入 TOML 文件。
    pub fn save(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        let config_path = self.config_dir.join("app_config.toml");
        let content = toml::to_string_pretty(&self.config)?;
        std::fs::write(&config_path, content)?;
        tracing::debug!("[Config] Config saved to {}", config_path.display());
        Ok(())
    }

    pub fn get(&self) -> &AppConfig {
        &self.config
    }

    pub fn get_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    /// 更新配置并持久化到文件。
    pub fn update(&mut self, config: AppConfig) -> AppResult<()> {
        tracing::info!("[Config] Config updated");
        self.config = config;
        self.save()
    }

    /// 重置为默认配置并持久化。
    pub fn reset_to_default(&mut self) -> AppResult<()> {
        tracing::warn!("[Config] Config reset to defaults");
        self.config = AppConfig::default();
        self.save()
    }

    /// 验证启动密码。
    ///
    /// 若未启用启动保护则始终返回 true。
    pub fn verify_startup_password(&self, password: &str) -> bool {
        if !self.config.security.enable_startup_protection {
            return true;
        }
        crate::crypto::CryptoService::verify_password(password, &self.config.security.password_hash)
    }

    /// 设置启动密码并启用启动保护。
    pub fn set_startup_password(&mut self, password: &str) -> AppResult<()> {
        tracing::info!("[Config] Startup password set, protection enabled");
        self.config.security.password_hash = crate::crypto::CryptoService::hash_password(password);
        self.config.security.enable_startup_protection = true;
        self.save()
    }

    /// 禁用启动保护并清除密码哈希。
    pub fn disable_startup_protection(&mut self) -> AppResult<()> {
        tracing::info!("[Config] Startup protection disabled");
        self.config.security.enable_startup_protection = false;
        self.config.security.password_hash = String::new();
        self.save()
    }

    /// 设置默认身份 ID。
    pub fn set_default_identity(&mut self, identity_id: i64) -> AppResult<()> {
        tracing::debug!("[Config] Default identity set to {}", identity_id);
        self.config.identity.default_identity_id = identity_id;
        self.save()
    }

    /// 清除默认身份设置。
    pub fn clear_default_identity(&mut self) -> AppResult<()> {
        tracing::debug!("[Config] Default identity cleared");
        self.config.identity.default_identity_id = 0;
        self.save()
    }

    /// 记录上次使用的身份 ID。
    pub fn set_last_identity(&mut self, identity_id: i64) -> AppResult<()> {
        tracing::debug!("[Config] Last identity set to {}", identity_id);
        self.config.identity.last_identity_id = identity_id;
        self.save()
    }

    /// 获取分类规则文件路径。
    ///
    /// 若配置中未指定则默认使用配置目录下的 `classification_rules.toml`。
    pub fn classification_rules_path(&self) -> PathBuf {
        if self.config.classification.rules_path.is_empty() {
            self.config_dir.join("classification_rules.toml")
        } else {
            PathBuf::from(&self.config.classification.rules_path)
        }
    }

    /// 获取数据目录路径。
    ///
    /// 若配置中未指定则默认使用 "Data"。
    pub fn data_directory(&self) -> PathBuf {
        if self.config.data.data_directory.is_empty() {
            PathBuf::from("Data")
        } else {
            PathBuf::from(&self.config.data.data_directory)
        }
    }

    /// 获取统计数值保留小数位数。
    pub fn decimal_places(&self) -> u32 {
        self.config.ui.decimal_places
    }

    /// 获取 ONNX 模型路径。
    ///
    /// 若配置中未指定则默认使用数据目录下的 `models` 子目录。
    pub fn onnx_model_path(&self) -> PathBuf {
        if self.config.captcha.onnx_model_path.is_empty() {
            self.data_directory().join("models")
        } else {
            PathBuf::from(&self.config.captcha.onnx_model_path)
        }
    }
}

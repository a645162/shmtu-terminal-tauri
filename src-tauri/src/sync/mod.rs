use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};

use shmtu_cas::captcha::CaptchaResolver;
use shmtu_cas::datatype::bill::BillType;
use shmtu_cas::sync::SyncOptions;

use crate::crypto::CryptoService;
use crate::db::{BillStoreImpl, DatabaseManager};
use crate::error::{AppError, AppResult};
use crate::models::Account;

/// 同步进度回调
pub type SyncProgressCallback = Box<dyn Fn(SyncProgress) + Send + Sync>;

/// 同步进度信息
#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub current_account: String,
    pub account_index: usize,
    pub total_accounts: usize,
    pub status: SyncStatus,
}

/// 同步状态
#[derive(Debug, Clone)]
pub enum SyncStatus {
    /// 正在探测登录状态
    ProbingLogin,
    /// 正在获取验证码
    GettingCaptcha,
    /// 正在登录
    LoggingIn,
    /// 正在同步数据（当前页/总页数）
    Syncing { page: u32, total: u32 },
    /// 同步完成
    Completed,
    /// 同步失败
    Failed(String),
}

/// 单个账号的同步结果
#[derive(Debug, Clone)]
pub struct AccountSyncResult {
    pub account_id: String,
    pub account_name: String,
    pub new_count: usize,
    pub pages_fetched: u32,
    pub early_stopped: bool,
    pub error: Option<String>,
}

/// 身份级别的同步结果
#[derive(Debug, Clone)]
pub struct IdentitySyncResult {
    pub results: Vec<AccountSyncResult>,
    pub total_new_count: usize,
}

/// 登录 challenge 信息
#[derive(Debug, Clone)]
pub struct LoginChallenge {
    /// CAS execution token
    pub execution: String,
    /// Base64 编码的验证码图片（不含前缀）
    pub captcha_image_base64: String,
}

/// 账单同步服务
pub struct BillSyncService {
    db_manager: DatabaseManager,
    crypto: CryptoService,
}

/// 配置访问器 —— 按需从文件读取，只在 resolve_captcha 时用到
struct ConfigAccess {
    data_dir: std::path::PathBuf,
}

impl ConfigAccess {
    fn new(db_manager: &DatabaseManager) -> Self {
        Self {
            data_dir: db_manager.data_dir().to_path_buf(),
        }
    }

    fn captcha_mode(&self) -> crate::config::CaptchaMode {
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok();
        let config = content.and_then(|c| toml::from_str::<crate::config::AppConfig>(&c).ok());
        config
            .map(|c| c.captcha.mode.clone())
            .unwrap_or(crate::config::CaptchaMode::Manual)
    }

    fn remote_ocr_host(&self) -> String {
        self.captcha_config()
            .map(|c| c.remote_ocr_host.clone())
            .unwrap_or_default()
    }

    fn remote_ocr_port(&self) -> u16 {
        self.captcha_config()
            .map(|c| c.remote_ocr_port)
            .unwrap_or(0)
    }

    fn ocr_retry_count(&self) -> usize {
        self.captcha_config()
            .map(|c| c.ocr_retry_count)
            .unwrap_or(3)
    }

    fn captcha_config(&self) -> Option<crate::config::CaptchaConfig> {
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok()?;
        let config: crate::config::AppConfig = toml::from_str(&content).ok()?;
        Some(config.captcha)
    }
}

impl BillSyncService {
    /// 创建同步服务
    pub fn new(db_manager: DatabaseManager, crypto: CryptoService) -> Self {
        Self { db_manager, crypto }
    }

    /// 获取身份下所有启用的账号
    pub fn get_enabled_accounts_for_identity(&self, identity_id: i64) -> AppResult<Vec<Account>> {
        let accounts = self.db_manager.list_accounts_by_identity(identity_id)?;
        Ok(accounts
            .into_iter()
            .filter(|a| a.enable && a.enable_update)
            .collect())
    }

    /// 单账号增量同步
    pub async fn sync_account(
        &self,
        account: &Account,
        sync_options: &SyncOptions,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<AccountSyncResult> {
        tracing::info!("开始同步账号: {}", account.account_id);
        let mut result = AccountSyncResult {
            account_id: account.account_id.clone(),
            account_name: account.account_name.clone(),
            new_count: 0,
            pages_fetched: 0,
            early_stopped: false,
            error: None,
        };

        let _identity = match self.db_manager.get_identity(account.identity_id)? {
            Some(id) => id,
            None => {
                result.error = Some("身份不存在".to_string());
                tracing::error!("账号 {}: 身份 {} 不存在", account.account_id, account.identity_id);
                return Ok(result);
            }
        };

        let mut store = match BillStoreImpl::new(
            self.db_manager.clone_ref(),
            &account.account_id,
            account.identity_id,
        ) {
            Ok(s) => s,
            Err(e) => {
                result.error = Some(format!("创建存储失败: {}", e));
                tracing::error!("账号 {}: 创建存储失败 {}", account.account_id, e);
                return Ok(result);
            }
        };

        // 创建 EpayAuth 并登录
        tracing::debug!("账号 {}: 尝试登录", account.account_id);
        let epay = match self.login_epay(account, progress_callback).await {
            Ok(e) => {
                tracing::info!("账号 {}: 登录成功", account.account_id);
                e
            }
            Err(e) => {
                result.error = Some(format!("登录失败: {}", e));
                tracing::error!("账号 {}: 登录失败 - {}", account.account_id, e);
                return Ok(result);
            }
        };

        // 执行增量同步
        tracing::info!("账号 {}: 开始获取账单", account.account_id);
        match shmtu_cas::sync::incremental_sync(&epay, &mut store, sync_options).await {
            Ok(sync_result) => {
                tracing::info!(
                    "账号 {}: 同步完成 - 新增 {} 条, 获取 {} 页, 是否提前停止: {}",
                    account.account_id, sync_result.new_count, sync_result.pages_fetched, sync_result.early_stopped
                );
                result.new_count = sync_result.new_count;
                result.pages_fetched = sync_result.pages_fetched;
                result.early_stopped = sync_result.early_stopped;
                let _ = self.db_manager.update_account_last_sync(account.id);
            }
            Err(e) => {
                result.error = Some(format!("同步失败: {}", e));
                tracing::error!("账号 {}: 同步失败 - {}", account.account_id, e);
            }
        }

        if let Some(cb) = progress_callback {
            cb(SyncProgress {
                current_account: account.account_name.clone(),
                account_index: 0,
                total_accounts: 1,
                status: SyncStatus::Completed,
            });
        }

        Ok(result)
    }

    /// 身份级别增量同步
    pub async fn sync_identity(
        &self,
        identity_id: i64,
        sync_options: &SyncOptions,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!("开始同步身份 {} 的账号列表", identity_id);
        let accounts = self.get_enabled_accounts_for_identity(identity_id)?;
        let total = accounts.len();
        tracing::info!("身份 {}: 找到 {} 个启用的账号", identity_id, total);

        let mut results = Vec::new();
        let mut total_new = 0;

        for (idx, account) in accounts.iter().enumerate() {
            if let Some(cb) = progress_callback {
                cb(SyncProgress {
                    current_account: account.account_name.clone(),
                    account_index: idx,
                    total_accounts: total,
                    status: SyncStatus::ProbingLogin,
                });
            }

            let account_result = self
                .sync_account(account, sync_options, progress_callback)
                .await?;
            total_new += account_result.new_count;
            results.push(account_result);
        }

        tracing::info!("身份 {}: 同步完成, 共新增 {} 条记录", identity_id, total_new);
        Ok(IdentitySyncResult {
            results,
            total_new_count: total_new,
        })
    }

    /// 单账号全量更新
    pub async fn full_sync_account(
        &self,
        account: &Account,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<AccountSyncResult> {
        self.db_manager
            .clear_account_original(&account.account_id)?;
        self.db_manager
            .clear_merged_by_account(account.identity_id, &account.account_id)?;
        self.db_manager
            .clear_operation_logs(account.identity_id, Some(&account.account_id))?;

        let sync_options = SyncOptions {
            start_page: 1,
            max_pages: 1000,
            bill_type: BillType::All,
            early_stop_threshold: u32::MAX,
        };

        self.sync_account(account, &sync_options, progress_callback)
            .await
    }

    /// 身份级别全量更新
    pub async fn full_sync_identity(
        &self,
        identity_id: i64,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        let accounts = self.get_enabled_accounts_for_identity(identity_id)?;

        self.db_manager.clear_merged_non_manual(identity_id)?;
        let _ = self.db_manager.clear_operation_logs(identity_id, None);

        let total = accounts.len();
        let mut results = Vec::new();
        let mut total_new = 0;

        for (idx, account) in accounts.iter().enumerate() {
            let _ = self.db_manager.clear_account_original(&account.account_id);

            if let Some(cb) = progress_callback {
                cb(SyncProgress {
                    current_account: account.account_name.clone(),
                    account_index: idx,
                    total_accounts: total,
                    status: SyncStatus::ProbingLogin,
                });
            }

            let sync_options = SyncOptions {
                start_page: 1,
                max_pages: 1000,
                bill_type: BillType::All,
                early_stop_threshold: u32::MAX,
            };

            let account_result = self
                .sync_account(account, &sync_options, progress_callback)
                .await?;
            total_new += account_result.new_count;
            results.push(account_result);
        }

        Ok(IdentitySyncResult {
            results,
            total_new_count: total_new,
        })
    }

    /// CAS 登录流程（优先尝试恢复 session，无效则重新登录）
    async fn login_epay(
        &self,
        account: &Account,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<EpayAuth> {
        // 先尝试从数据库加载 session
        if let Some(session) = self
            .db_manager
            .get_session(&account.account_id, &self.crypto)?
        {
            tracing::debug!("账号 {}: 尝试恢复已有 session", account.account_id);
            let mut epay = EpayAuth::new()?;
            epay.restore_session(&session.cookies)?;

            // 验证 session 是否仍然有效
            match epay.probe_login().await {
                Ok(LoginProbe::AlreadyLoggedIn) => {
                    tracing::info!("账号 {}: session 有效，直接复用", account.account_id);
                    return Ok(epay);
                }
                Ok(LoginProbe::NeedLogin { .. }) => {
                    tracing::info!("账号 {}: session 已过期，需要重新登录", account.account_id);
                }
                Err(e) => {
                    tracing::warn!("账号 {}: session 验证失败 - {}，将重新登录", account.account_id, e);
                }
            }
        } else {
            tracing::debug!("账号 {}: 未找到 session，需要重新登录", account.account_id);
        }

        // 无 session 或已失效，重新登录
        if let Some(cb) = progress_callback {
            cb(SyncProgress {
                current_account: account.account_name.clone(),
                account_index: 0,
                total_accounts: 1,
                status: SyncStatus::ProbingLogin,
            });
        }

        let password = self
            .db_manager
            .decrypt_account_password(account, &self.crypto)?;

        let epay = self
            .login_with_retry(&account.account_id, &password, progress_callback)
            .await?;

        // 登录成功后保存 session
        self.save_session(&epay, &account.account_id).await?;

        Ok(epay)
    }

    /// 带重试的登录流程（自动处理验证码）
    async fn login_with_retry(
        &self,
        username: &str,
        password: &str,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<EpayAuth> {
        // 手动模式不支持后台同步
        let cfg = ConfigAccess::new(&self.db_manager);
        if matches!(cfg.captcha_mode(), crate::config::CaptchaMode::Manual) {
            return Err(AppError::Sync(
                "手动模式需要前端交互，请在应用中手动输入验证码".to_string(),
            ));
        }

        loop {
            let mut epay = EpayAuth::new()?;
            epay.probe_login().await?;

            if let Some(cb) = progress_callback {
                cb(SyncProgress {
                    current_account: username.to_string(),
                    account_index: 0,
                    total_accounts: 1,
                    status: SyncStatus::GettingCaptcha,
                });
            }

            let challenge = epay.prepare_challenge().await?;

            if let Some(cb) = progress_callback {
                cb(SyncProgress {
                    current_account: username.to_string(),
                    account_index: 0,
                    total_accounts: 1,
                    status: SyncStatus::LoggingIn,
                });
            }

            let validate_code = match self.resolve_captcha(&challenge.captcha_image).await {
                Ok(code) => code,
                Err(e) => {
                    tracing::warn!("验证码识别失败: {}，重新获取验证码", e);
                    continue;
                }
            };

            match epay
                .submit_login(username, password, &validate_code, &challenge.execution)
                .await?
            {
                LoginSubmitResult::Success => {
                    if epay.test_login_status().await? {
                        return Ok(epay);
                    } else {
                        tracing::warn!("登录验证失败，重试中...");
                        continue;
                    }
                }
                LoginSubmitResult::ValidateCodeError => {
                    tracing::warn!("验证码错误，重试中...");
                    continue;
                }
                LoginSubmitResult::PasswordError => {
                    return Err(AppError::Sync("用户名或密码错误".to_string()));
                }
                LoginSubmitResult::Failure(msg) => {
                    return Err(AppError::Sync(format!("登录失败: {}", msg)));
                }
            }
        }
    }

    /// 使用预登录状态和验证码完成登录（前端回调）
    pub async fn login_with_captcha(
        &self,
        account: &Account,
        validate_code: &str,
    ) -> AppResult<EpayAuth> {
        let mut epay = EpayAuth::new()?;
        let password = self
            .db_manager
            .decrypt_account_password(account, &self.crypto)?;

        epay.probe_login().await?;
        let challenge = epay.prepare_challenge().await?;

        match epay
            .submit_login(
                &account.account_id,
                &password,
                validate_code,
                &challenge.execution,
            )
            .await?
        {
            LoginSubmitResult::Success => {
                self.save_session(&epay, &account.account_id).await?;
                Ok(epay)
            }
            LoginSubmitResult::ValidateCodeError => Err(AppError::Sync("验证码错误".to_string())),
            LoginSubmitResult::PasswordError => Err(AppError::Sync("密码错误".to_string())),
            LoginSubmitResult::Failure(msg) => Err(AppError::Sync(format!("登录失败: {}", msg))),
        }
    }

    /// 获取当前预登录状态（验证码图片 + execution）
    pub fn get_pre_login_state(&self) -> Option<LoginChallenge> {
        // 此方法已被移除，前端应直接使用 commands/captcha.rs 中的 get_captcha_with_execution
        None
    }

    /// 自动识别验证码（使用配置的 OCR 模式）
    async fn resolve_captcha(&self, image_data: &[u8]) -> AppResult<String> {
        use crate::config::CaptchaMode;

        let cfg = ConfigAccess::new(&self.db_manager);

        match cfg.captcha_mode() {
            CaptchaMode::Manual => Err(AppError::Sync("手动模式需要前端交互".to_string())),
            CaptchaMode::RemoteOcr => {
                let host = cfg.remote_ocr_host();
                let port = cfg.remote_ocr_port();
                let retry_count = cfg.ocr_retry_count();

                if host.is_empty() || port == 0 {
                    return Err(AppError::Sync("未配置远程OCR服务器".to_string()));
                }

                tracing::debug!("正在使用远程OCR识别验证码: {}:{}", host, port);
                let resolver = shmtu_cas::captcha::OcrCaptchaResolver::new(&host, port)
                    .with_retries(retry_count);

                let result = resolver.resolve(image_data).await?;
                let answer = result.into_final_answer();
                tracing::info!("验证码识别成功: {}", answer);
                Ok(answer)
            }
            CaptchaMode::LocalOnnx => Err(AppError::Sync("本地ONNX模式暂未实现".to_string())),
        }
    }

    /// 保存会话 cookies 到数据库
    async fn save_session(&self, epay: &EpayAuth, account_id: &str) -> AppResult<()> {
        let cookies_json = epay.extract_session()?;
        self.db_manager
            .save_session(account_id, &cookies_json, &self.crypto)?;
        Ok(())
    }
}

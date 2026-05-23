use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use shmtu_cas::captcha::CaptchaResolver;
use shmtu_cas::datatype::bill::BillType;
use shmtu_cas::sync::SyncOptions;

use crate::crypto::CryptoService;
use crate::db::{BillStoreImpl, DatabaseManager};
use crate::error::{AppError, AppResult};
use crate::models::Account;

pub type SyncProgressCallback = Box<dyn Fn(SyncProgress) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub current_account: String,
    pub account_index: usize,
    pub total_accounts: usize,
    pub status: SyncStatus,
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    ProbingLogin,
    GettingCaptcha,
    LoggingIn,
    Syncing { page: u32, total: u32 },
    Completed,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct AccountSyncResult {
    pub account_id: String,
    pub account_name: String,
    pub new_count: usize,
    pub pages_fetched: u32,
    pub early_stopped: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IdentitySyncResult {
    pub results: Vec<AccountSyncResult>,
    pub total_new_count: usize,
}

pub struct BillSyncService {
    db_manager: DatabaseManager,
    crypto: CryptoService,
}

struct ConfigAccess {
    data_dir: std::path::PathBuf,
}

impl ConfigAccess {
    fn new(db_manager: &DatabaseManager) -> Self {
        Self { data_dir: db_manager.data_dir().to_path_buf() }
    }

    fn captcha_mode(&self) -> crate::config::CaptchaMode {
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok();
        let config = content.and_then(|c| toml::from_str::<crate::config::AppConfig>(&c).ok());
        config.map(|c| c.captcha.mode.clone()).unwrap_or(crate::config::CaptchaMode::Manual)
    }

    fn remote_ocr_host(&self) -> String {
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok();
        let config = content.and_then(|c| toml::from_str::<crate::config::AppConfig>(&c).ok());
        config.map(|c| c.captcha.remote_ocr_host.clone()).unwrap_or_default()
    }

    fn remote_ocr_port(&self) -> u16 {
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok();
        let config = content.and_then(|c| toml::from_str::<crate::config::AppConfig>(&c).ok());
        config.map(|c| c.captcha.remote_ocr_port).unwrap_or(0)
    }
}

impl BillSyncService {
    pub fn new(db_manager: DatabaseManager, crypto: CryptoService) -> Self {
        Self { db_manager, crypto }
    }

    pub fn get_enabled_accounts_for_identity(&self, identity_id: i64) -> AppResult<Vec<Account>> {
        let accounts = self.db_manager.list_accounts_by_identity(identity_id)?;
        Ok(accounts.into_iter().filter(|a| a.enable && a.enable_update).collect())
    }

    pub async fn sync_identity(&self, identity_id: i64, progress_callback: Option<&SyncProgressCallback>) -> AppResult<IdentitySyncResult> {
        tracing::info!("[Sync] sync_identity called, identity_id={}", identity_id);
        let cfg = ConfigAccess::new(&self.db_manager);
        
        if matches!(cfg.captcha_mode(), crate::config::CaptchaMode::Manual) {
            tracing::info!("[Sync] Manual mode detected");
            let (image, execution) = self.get_captcha_for_manual_login().await?;
            return Err(AppError::Sync(format!("MANUAL_CAPTCHA_REQUIRED|{}|{}", image, execution)));
        }
        
        let sync_options = SyncOptions {
            start_page: 1,
            max_pages: 100,
            bill_type: BillType::All,
            early_stop_threshold: 10,
        };
        
        self.do_sync(identity_id, &sync_options, progress_callback).await
    }

    async fn do_sync(&self, identity_id: i64, sync_options: &SyncOptions, progress_callback: Option<&SyncProgressCallback>) -> AppResult<IdentitySyncResult> {
        let accounts = self.get_enabled_accounts_for_identity(identity_id)?;
        let mut results = Vec::new();
        let mut total_new = 0;

        for (idx, account) in accounts.iter().enumerate() {
            if let Some(cb) = progress_callback {
                cb(SyncProgress { current_account: account.account_name.clone(), account_index: idx, total_accounts: accounts.len(), status: SyncStatus::ProbingLogin });
            }
            let account_result = self.sync_single_account(account, sync_options, progress_callback).await?;
            total_new += account_result.new_count;
            results.push(account_result);
        }

        Ok(IdentitySyncResult { results, total_new_count: total_new })
    }

    async fn sync_single_account(&self, account: &Account, sync_options: &SyncOptions, _progress_callback: Option<&SyncProgressCallback>) -> AppResult<AccountSyncResult> {
        tracing::info!("[Sync] sync_single_account: {}", account.account_id);
        let mut result = AccountSyncResult { account_id: account.account_id.clone(), account_name: account.account_name.clone(), new_count: 0, pages_fetched: 0, early_stopped: false, error: None };

        // 尝试恢复 session
        if let Some(session) = self.db_manager.get_session(&account.account_id, &self.crypto)? {
            let mut epay = EpayAuth::new()?;
            epay.restore_session(&session.cookies)?;
            if let Ok(LoginProbe::AlreadyLoggedIn) = epay.probe_login().await {
                tracing::info!("[Sync] Session valid");
                let mut store = BillStoreImpl::new(self.db_manager.clone_ref(), &account.account_id, account.identity_id)?;
                match shmtu_cas::sync::incremental_sync(&epay, &mut store, sync_options).await {
                    Ok(sync_result) => {
                        result.new_count = sync_result.new_count;
                        result.pages_fetched = sync_result.pages_fetched;
                        result.early_stopped = sync_result.early_stopped;
                        let _ = self.db_manager.update_account_last_sync(account.id);
                    }
                    Err(e) => result.error = Some(format!("同步失败: {}", e)),
                }
                return Ok(result);
            }
        }

        // 需要登录
        let password = self.db_manager.decrypt_account_password(account, &self.crypto)?;
        let epay = self.login_auto(&account.account_id, &password).await?;
        self.save_session(&epay, &account.account_id).await?;

        let mut store = BillStoreImpl::new(self.db_manager.clone_ref(), &account.account_id, account.identity_id)?;
        match shmtu_cas::sync::incremental_sync(&epay, &mut store, sync_options).await {
            Ok(sync_result) => {
                result.new_count = sync_result.new_count;
                result.pages_fetched = sync_result.pages_fetched;
                result.early_stopped = sync_result.early_stopped;
                let _ = self.db_manager.update_account_last_sync(account.id);
            }
            Err(e) => result.error = Some(format!("同步失败: {}", e)),
        }

        Ok(result)
    }

    async fn login_auto(&self, username: &str, password: &str) -> AppResult<EpayAuth> {
        let cfg = ConfigAccess::new(&self.db_manager);
        
        for attempt in 1..=3 {
            tracing::info!("[Sync] Login attempt {}/3", attempt);
            let mut epay = EpayAuth::new()?;
            epay.probe_login().await?;
            let challenge = epay.prepare_challenge().await?;

            let host = cfg.remote_ocr_host();
            let port = cfg.remote_ocr_port();
            if host.is_empty() || port == 0 {
                return Err(AppError::Sync("未配置远程OCR服务器".to_string()));
            }
            
            tracing::info!("[Sync] Using remote OCR {}:{}", host, port);
            let captcha_code = shmtu_cas::captcha::OcrCaptchaResolver::new(&host, port)
                .with_retries(3)
                .resolve(&challenge.captcha_image).await?
                .into_final_answer();

            tracing::info!("[Sync] Captcha: {}", captcha_code);

            match epay.submit_login(username, password, &captcha_code, &challenge.execution).await? {
                LoginSubmitResult::Success => {
                    if let Ok(LoginProbe::AlreadyLoggedIn) = epay.probe_login().await {
                        tracing::info!("[Sync] Login SUCCESS");
                        return Ok(epay);
                    }
                }
                LoginSubmitResult::ValidateCodeError => {
                    tracing::warn!("[Sync] Captcha wrong, retry {}/3", attempt);
                    if attempt < 3 { continue; }
                    return Err(AppError::Sync("验证码识别多次失败".to_string()));
                }
                LoginSubmitResult::PasswordError => return Err(AppError::Sync("用户名或密码错误".to_string())),
                LoginSubmitResult::Failure(msg) => return Err(AppError::Sync(format!("登录失败: {}", msg))),
            }
        }
        Err(AppError::Sync("登录重试次数耗尽".to_string()))
    }

    pub async fn sync_with_captcha(&self, identity_id: i64, captcha_code: &str, execution: &str) -> AppResult<IdentitySyncResult> {
        tracing::info!("[Sync] sync_with_captcha, identity_id={}, captcha={}", identity_id, captcha_code);
        let accounts = self.get_enabled_accounts_for_identity(identity_id)?;
        let mut total_new = 0;
        let mut results = Vec::new();

        for account in accounts {
            tracing::info!("[Sync] Processing: {}", account.account_id);
            let mut epay = EpayAuth::new()?;
            epay.probe_login().await?;
            let password = self.db_manager.decrypt_account_password(&account, &self.crypto)?;

            match epay.submit_login(&account.account_id, &password, captcha_code, execution).await? {
                LoginSubmitResult::Success => {
                    tracing::info!("[Sync] Login OK");
                    self.save_session(&epay, &account.account_id).await?;
                    let mut store = BillStoreImpl::new(self.db_manager.clone_ref(), &account.account_id, account.identity_id)?;
                    let sync_options = SyncOptions { start_page: 1, max_pages: 100, bill_type: BillType::All, early_stop_threshold: 10 };
                    
                    match shmtu_cas::sync::incremental_sync(&epay, &mut store, &sync_options).await {
                        Ok(sync_result) => {
                            let _ = self.db_manager.update_account_last_sync(account.id);
                            total_new += sync_result.new_count;
                            results.push(AccountSyncResult { account_id: account.account_id.clone(), account_name: account.account_name.clone(), new_count: sync_result.new_count, pages_fetched: sync_result.pages_fetched, early_stopped: sync_result.early_stopped, error: None });
                        }
                        Err(e) => results.push(AccountSyncResult { account_id: account.account_id.clone(), account_name: account.account_name.clone(), new_count: 0, pages_fetched: 0, early_stopped: false, error: Some(format!("同步失败: {}", e)) }),
                    }
                }
                LoginSubmitResult::ValidateCodeError => {
                    tracing::error!("[Sync] Captcha WRONG!");
                    let (new_image, new_exec) = self.get_captcha_for_manual_login().await?;
                    return Err(AppError::Sync(format!("CAPTCHA_WRONG|{}|{}", new_image, new_exec)));
                }
                LoginSubmitResult::PasswordError => return Err(AppError::Sync("用户名或密码错误".to_string())),
                LoginSubmitResult::Failure(msg) => return Err(AppError::Sync(format!("登录失败: {}", msg))),
            }
        }

        tracing::info!("[Sync] Done, new={}", total_new);
        Ok(IdentitySyncResult { results, total_new_count: total_new })
    }

    pub async fn login_with_captcha(&self, account: &Account, validate_code: &str) -> AppResult<EpayAuth> {
        let mut epay = EpayAuth::new()?;
        let password = self.db_manager.decrypt_account_password(account, &self.crypto)?;
        epay.probe_login().await?;
        let challenge = epay.prepare_challenge().await?;

        match epay.submit_login(&account.account_id, &password, validate_code, &challenge.execution).await? {
            LoginSubmitResult::Success => { self.save_session(&epay, &account.account_id).await?; Ok(epay) }
            LoginSubmitResult::ValidateCodeError => Err(AppError::Sync("验证码错误".to_string())),
            LoginSubmitResult::PasswordError => Err(AppError::Sync("用户名或密码错误".to_string())),
            LoginSubmitResult::Failure(msg) => Err(AppError::Sync(format!("登录失败: {}", msg))),
        }
    }

    pub async fn get_captcha_for_manual_login(&self) -> AppResult<(String, String)> {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        tracing::info!("[Sync] get_captcha_for_manual_login");
        let mut epay = EpayAuth::new()?;
        epay.probe_login().await?;
        let challenge = epay.prepare_challenge().await?;
        let base64_image = BASE64.encode(&challenge.captcha_image);
        Ok((base64_image, challenge.execution))
    }

    async fn save_session(&self, epay: &EpayAuth, account_id: &str) -> AppResult<()> {
        let cookies_json = epay.extract_session()?;
        self.db_manager.save_session(account_id, &cookies_json, &self.crypto)?;
        Ok(())
    }

    pub async fn full_sync_identity(&self, identity_id: i64, progress_callback: Option<&SyncProgressCallback>) -> AppResult<IdentitySyncResult> {
        tracing::info!("[Sync] full_sync_identity, identity_id={}", identity_id);
        self.db_manager.clear_merged_non_manual(identity_id)?;
        let _ = self.db_manager.clear_operation_logs(identity_id, None);
        let accounts = self.get_enabled_accounts_for_identity(identity_id)?;
        let mut results = Vec::new();
        let mut total_new = 0;

        for (idx, account) in accounts.iter().enumerate() {
            let _ = self.db_manager.clear_account_original(&account.account_id);
            if let Some(cb) = progress_callback {
                cb(SyncProgress { current_account: account.account_name.clone(), account_index: idx, total_accounts: accounts.len(), status: SyncStatus::ProbingLogin });
            }
            let sync_options = SyncOptions { start_page: 1, max_pages: 1000, bill_type: BillType::All, early_stop_threshold: u32::MAX };
            let account_result = self.sync_single_account(account, &sync_options, progress_callback).await?;
            total_new += account_result.new_count;
            results.push(account_result);
        }

        Ok(IdentitySyncResult { results, total_new_count: total_new })
    }
}

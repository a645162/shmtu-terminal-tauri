use std::collections::VecDeque;

use shmtu_cas::captcha::CaptchaResolver;
use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use shmtu_cas::classifier::PositionTranslator;
use shmtu_cas::datatype::bill::BillType;
use shmtu_cas::sync::SyncOptions;
use tokio::sync::Mutex;

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
    pending_manual_sync: Mutex<Option<PendingManualSync>>,
    translator: PositionTranslator,
}

struct PendingManualSync {
    identity_id: i64,
    sync_options: SyncOptions,
    current_account: Account,
    remaining_accounts: VecDeque<Account>,
    results: Vec<AccountSyncResult>,
    total_new_count: usize,
    epay: EpayAuth,
    execution: String,
}

impl PendingManualSync {
    fn push_result(&mut self, result: AccountSyncResult) {
        self.total_new_count += result.new_count;
        self.results.push(result);
    }
}

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
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok();
        let config = content.and_then(|c| toml::from_str::<crate::config::AppConfig>(&c).ok());
        config
            .map(|c| c.captcha.remote_ocr_host.clone())
            .unwrap_or_default()
    }

    fn remote_ocr_port(&self) -> u16 {
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok();
        let config = content.and_then(|c| toml::from_str::<crate::config::AppConfig>(&c).ok());
        config.map(|c| c.captcha.remote_ocr_port).unwrap_or(0)
    }
}

impl BillSyncService {
    pub fn new(db_manager: DatabaseManager, crypto: CryptoService, translator: PositionTranslator) -> Self {
        Self {
            db_manager,
            crypto,
            pending_manual_sync: Mutex::new(None),
            translator,
        }
    }

    pub async fn get_enabled_accounts_for_identity(&self, identity_id: i64) -> AppResult<Vec<Account>> {
        let accounts = self.db_manager.list_accounts_by_identity(identity_id).await?;
        Ok(accounts
            .into_iter()
            .filter(|a| a.enable && a.enable_update)
            .collect())
    }

    pub async fn sync_identity(
        &self,
        identity_id: i64,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!("[Sync] sync_identity called, identity_id={}", identity_id);
        let cfg = ConfigAccess::new(&self.db_manager);
        let sync_options = Self::default_incremental_sync_options();

        if matches!(cfg.captcha_mode(), crate::config::CaptchaMode::Manual) {
            tracing::info!("[Sync] Manual mode detected");
            self.clear_pending_manual_sync().await;
            return self
                .sync_identity_manual(identity_id, sync_options, progress_callback)
                .await;
        }

        self.do_sync(identity_id, &sync_options, progress_callback)
            .await
    }

    async fn do_sync(
        &self,
        identity_id: i64,
        sync_options: &SyncOptions,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        let accounts = self.get_enabled_accounts_for_identity(identity_id).await?;
        let mut results = Vec::new();
        let mut total_new = 0;

        for (idx, account) in accounts.iter().enumerate() {
            if let Some(cb) = progress_callback {
                cb(SyncProgress {
                    current_account: account.account_name.clone(),
                    account_index: idx,
                    total_accounts: accounts.len(),
                    status: SyncStatus::ProbingLogin,
                });
            }
            let account_result = self
                .sync_single_account(account, sync_options, progress_callback)
                .await?;
            total_new += account_result.new_count;
            results.push(account_result);
        }

        Ok(IdentitySyncResult {
            results,
            total_new_count: total_new,
        })
    }

    async fn sync_single_account(
        &self,
        account: &Account,
        sync_options: &SyncOptions,
        _progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<AccountSyncResult> {
        tracing::info!("[Sync] sync_single_account: {}", account.account_id);

        if let Some(cached_result) = self
            .try_sync_with_saved_session(account, sync_options)
            .await?
        {
            return Ok(cached_result);
        }

        let password = self
            .db_manager
            .decrypt_account_password(account, &self.crypto)?;
        let epay = self.login_auto(&account.account_id, &password).await?;
        self.save_session(&epay, &account.account_id).await?;

        self.sync_logged_in_account(account, &epay, sync_options)
            .await
    }

    async fn sync_identity_manual(
        &self,
        identity_id: i64,
        sync_options: SyncOptions,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        let accounts = self.get_enabled_accounts_for_identity(identity_id).await?;
        let total_accounts = accounts.len();
        let mut remaining_accounts: VecDeque<Account> = accounts.into();
        let mut results = Vec::new();
        let mut total_new_count = 0usize;
        let mut processed_accounts = 0usize;

        while let Some(account) = remaining_accounts.pop_front() {
            if let Some(cb) = progress_callback {
                cb(SyncProgress {
                    current_account: account.account_name.clone(),
                    account_index: processed_accounts,
                    total_accounts,
                    status: SyncStatus::ProbingLogin,
                });
            }

            if let Some(result) = self
                .try_sync_with_saved_session(&account, &sync_options)
                .await?
            {
                total_new_count += result.new_count;
                results.push(result);
                processed_accounts += 1;
                continue;
            }

            let mut epay = EpayAuth::new()?;
            match epay.probe_login().await? {
                LoginProbe::AlreadyLoggedIn => {
                    let result = self
                        .sync_logged_in_account(&account, &epay, &sync_options)
                        .await?;
                    total_new_count += result.new_count;
                    results.push(result);
                    processed_accounts += 1;
                }
                LoginProbe::NeedLogin { .. } => {
                    let challenge = epay.prepare_challenge().await?;
                    let image = Self::encode_captcha_image(&challenge.captcha_image);
                    let execution = challenge.execution.clone();

                    self.store_pending_manual_sync(PendingManualSync {
                        identity_id,
                        sync_options,
                        current_account: account,
                        remaining_accounts,
                        results,
                        total_new_count,
                        epay,
                        execution: execution.clone(),
                    })
                    .await;

                    return Err(AppError::Sync(format!(
                        "MANUAL_CAPTCHA_REQUIRED|{}|{}",
                        image, execution
                    )));
                }
            }
        }

        Ok(IdentitySyncResult {
            results,
            total_new_count,
        })
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
                .resolve(&challenge.captcha_image)
                .await?
                .into_final_answer();

            tracing::info!("[Sync] Captcha: {}", captcha_code);

            match epay
                .submit_login(username, password, &captcha_code, &challenge.execution)
                .await?
            {
                LoginSubmitResult::Success => {
                    if let Ok(LoginProbe::AlreadyLoggedIn) = epay.probe_login().await {
                        tracing::info!("[Sync] Login SUCCESS");
                        return Ok(epay);
                    }
                }
                LoginSubmitResult::ValidateCodeError => {
                    tracing::warn!("[Sync] Captcha wrong, retry {}/3", attempt);
                    if attempt < 3 {
                        continue;
                    }
                    return Err(AppError::Sync("验证码识别多次失败".to_string()));
                }
                LoginSubmitResult::PasswordError => {
                    return Err(AppError::Sync("用户名或密码错误".to_string()));
                }
                LoginSubmitResult::Failure(msg) => {
                    return Err(AppError::Sync(format!("登录失败: {}", msg)));
                }
            }
        }

        Err(AppError::Sync("登录重试次数耗尽".to_string()))
    }

    pub async fn sync_with_captcha(
        &self,
        identity_id: i64,
        captcha_code: &str,
        execution: &str,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!(
            "[Sync] sync_with_captcha, identity_id={}, captcha={}, execution_len={}",
            identity_id,
            captcha_code,
            execution.len()
        );

        let mut pending = self
            .take_pending_manual_sync()
            .await
            .ok_or_else(|| AppError::Sync("当前没有待处理的验证码登录".to_string()))?;

        if pending.identity_id != identity_id {
            self.store_pending_manual_sync(pending).await;
            return Err(AppError::Sync(
                "待处理的验证码身份不匹配，请重新开始同步".to_string(),
            ));
        }

        let password = self
            .db_manager
            .decrypt_account_password(&pending.current_account, &self.crypto)?;

        match pending
            .epay
            .submit_login(
                &pending.current_account.account_id,
                &password,
                captcha_code,
                &pending.execution,
            )
            .await?
        {
            LoginSubmitResult::Success => {
                tracing::info!("[Sync] Login OK for {}", pending.current_account.account_id);
                self.save_session(&pending.epay, &pending.current_account.account_id)
                    .await?;
                let result = self
                    .sync_logged_in_account(
                        &pending.current_account,
                        &pending.epay,
                        &pending.sync_options,
                    )
                    .await?;
                pending.push_result(result);
                self.continue_pending_manual_sync(pending).await
            }
            LoginSubmitResult::ValidateCodeError => {
                tracing::error!("[Sync] Captcha WRONG!");
                let challenge = pending.epay.prepare_challenge().await?;
                let image = Self::encode_captcha_image(&challenge.captcha_image);
                pending.execution = challenge.execution.clone();
                let execution = pending.execution.clone();
                self.store_pending_manual_sync(pending).await;
                Err(AppError::Sync(format!(
                    "CAPTCHA_WRONG|{}|{}",
                    image, execution
                )))
            }
            LoginSubmitResult::PasswordError => Err(AppError::Sync("用户名或密码错误".to_string())),
            LoginSubmitResult::Failure(msg) => Err(AppError::Sync(format!("登录失败: {}", msg))),
        }
    }

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
            LoginSubmitResult::PasswordError => Err(AppError::Sync("用户名或密码错误".to_string())),
            LoginSubmitResult::Failure(msg) => Err(AppError::Sync(format!("登录失败: {}", msg))),
        }
    }

    pub async fn get_captcha_for_manual_login(&self) -> AppResult<(String, String)> {
        tracing::info!("[Sync] get_captcha_for_manual_login");

        if let Some(mut pending) = self.take_pending_manual_sync().await {
            let challenge = pending.epay.prepare_challenge().await?;
            let image = Self::encode_captcha_image(&challenge.captcha_image);
            pending.execution = challenge.execution.clone();
            let execution = pending.execution.clone();
            self.store_pending_manual_sync(pending).await;
            return Ok((image, execution));
        }

        let mut epay = EpayAuth::new()?;
        epay.probe_login().await?;
        let challenge = epay.prepare_challenge().await?;
        Ok((
            Self::encode_captcha_image(&challenge.captcha_image),
            challenge.execution,
        ))
    }

    async fn save_session(&self, epay: &EpayAuth, account_id: &str) -> AppResult<()> {
        let cookies_json = epay.extract_session()?;
        self.db_manager.save_session(account_id, &cookies_json, &self.crypto).await?;
        Ok(())
    }

    pub async fn full_sync_identity(
        &self,
        identity_id: i64,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!("[Sync] full_sync_identity, identity_id={}", identity_id);
        // 全量更新：清空旧数据
        self.db_manager.clear_merged_non_manual(identity_id).await?;
        let _ = self.db_manager.clear_operation_logs(identity_id, None).await;
        let accounts = self.get_enabled_accounts_for_identity(identity_id).await?;
        let mut results = Vec::new();
        let mut total_new = 0;

        for (idx, account) in accounts.iter().enumerate() {
            // 清除该账号的原始数据和 session
            let _ = self.db_manager.clear_account_original(&account.account_id).await;
            // 全量更新时清除旧 session，强制重新登录
            let _ = self.db_manager.invalidate_session(&account.account_id).await;
            if let Some(cb) = progress_callback {
                cb(SyncProgress {
                    current_account: account.account_name.clone(),
                    account_index: idx,
                    total_accounts: accounts.len(),
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
                .sync_single_account(account, &sync_options, progress_callback)
                .await?;
            total_new += account_result.new_count;
            results.push(account_result);
        }

        Ok(IdentitySyncResult {
            results,
            total_new_count: total_new,
        })
    }

    /// 增量同步单个账号
    pub async fn sync_single_account_by_id(
        &self,
        identity_id: i64,
        account_id: &str,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!("[Sync] sync_single_account_by_id, identity_id={}, account_id={}", identity_id, account_id);
        let accounts = self.get_enabled_accounts_for_identity(identity_id).await?;
        let account = accounts
            .into_iter()
            .find(|a| a.account_id == account_id)
            .ok_or_else(|| AppError::Sync(format!("账号 {} 不存在或已禁用", account_id)))?;

        let sync_options = Self::default_incremental_sync_options();
        let sync_options = SyncOptions {
            start_page: sync_options.start_page,
            max_pages: sync_options.max_pages,
            bill_type: BillType::All,
            early_stop_threshold: sync_options.early_stop_threshold,
        };
        let account_result = self.sync_single_account(&account, &sync_options, progress_callback).await?;

        Ok(IdentitySyncResult {
            total_new_count: account_result.new_count,
            results: vec![account_result],
        })
    }

    /// 全量同步单个账号（清空旧数据 + 清空旧 session 后重新同步）
    pub async fn full_sync_single_account(
        &self,
        identity_id: i64,
        account_id: &str,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!("[Sync] full_sync_single_account, identity_id={}, account_id={}", identity_id, account_id);
        let accounts = self.get_enabled_accounts_for_identity(identity_id).await?;
        let account = accounts
            .into_iter()
            .find(|a| a.account_id == account_id)
            .ok_or_else(|| AppError::Sync(format!("账号 {} 不存在或已禁用", account_id)))?;

        // 全量更新：清除该账号的旧数据和 session
        let _ = self.db_manager.clear_account_original(&account.account_id).await;
        // 清除旧 session，强制重新登录
        let _ = self.db_manager.invalidate_session(&account.account_id).await;
        // 清除该账号在合并表中的相关记录（非手动）
        let _ = self.db_manager.clear_merged_by_account(identity_id, &account.account_id).await;

        let sync_options = SyncOptions {
            start_page: 1,
            max_pages: 1000,
            bill_type: BillType::All,
            early_stop_threshold: u32::MAX,
        };
        let account_result = self.sync_single_account(&account, &sync_options, progress_callback).await?;

        Ok(IdentitySyncResult {
            total_new_count: account_result.new_count,
            results: vec![account_result],
        })
    }

    async fn continue_pending_manual_sync(
        &self,
        mut pending: PendingManualSync,
    ) -> AppResult<IdentitySyncResult> {
        while let Some(account) = pending.remaining_accounts.pop_front() {
            tracing::info!("[Sync] Processing: {}", account.account_id);

            if let Some(result) = self
                .try_sync_with_saved_session(&account, &pending.sync_options)
                .await?
            {
                pending.push_result(result);
                continue;
            }

            let mut epay = EpayAuth::new()?;
            match epay.probe_login().await? {
                LoginProbe::AlreadyLoggedIn => {
                    let result = self
                        .sync_logged_in_account(&account, &epay, &pending.sync_options)
                        .await?;
                    pending.push_result(result);
                }
                LoginProbe::NeedLogin { .. } => {
                    let challenge = epay.prepare_challenge().await?;
                    let image = Self::encode_captcha_image(&challenge.captcha_image);
                    pending.current_account = account;
                    pending.epay = epay;
                    pending.execution = challenge.execution.clone();
                    let execution = pending.execution.clone();
                    self.store_pending_manual_sync(pending).await;
                    return Err(AppError::Sync(format!(
                        "MANUAL_CAPTCHA_REQUIRED|{}|{}",
                        image, execution
                    )));
                }
            }
        }

        tracing::info!("[Sync] Done, new={}", pending.total_new_count);
        Ok(IdentitySyncResult {
            results: pending.results,
            total_new_count: pending.total_new_count,
        })
    }

    async fn try_sync_with_saved_session(
        &self,
        account: &Account,
        sync_options: &SyncOptions,
    ) -> AppResult<Option<AccountSyncResult>> {
        if let Some(session) = self.db_manager.get_session(&account.account_id, &self.crypto).await?
        {
            let mut epay = EpayAuth::new()?;
            epay.restore_session(&session.cookies)?;
            if let Ok(LoginProbe::AlreadyLoggedIn) = epay.probe_login().await {
                tracing::info!("[Sync] Session valid for {}", account.account_id);
                return Ok(Some(
                    self.sync_logged_in_account(account, &epay, sync_options)
                        .await?,
                ));
            }
        }

        Ok(None)
    }

    async fn sync_logged_in_account(
        &self,
        account: &Account,
        epay: &EpayAuth,
        sync_options: &SyncOptions,
    ) -> AppResult<AccountSyncResult> {
        let mut store = BillStoreImpl::new(
            self.db_manager.db().clone(),
            &account.account_id,
            account.identity_id,
            self.translator.clone(),
        ).await?;
        let mut result = AccountSyncResult {
            account_id: account.account_id.clone(),
            account_name: account.account_name.clone(),
            new_count: 0,
            pages_fetched: 0,
            early_stopped: false,
            error: None,
        };

        match shmtu_cas::sync::incremental_sync(epay, &mut store, sync_options).await {
            Ok(sync_result) => {
                store.flush_pending_bills().await?;
                result.new_count = sync_result.new_count;
                result.pages_fetched = sync_result.pages_fetched;
                result.early_stopped = sync_result.early_stopped;
                let _ = self.db_manager.update_account_last_sync(account.id).await;
            }
            Err(e) => {
                result.error = Some(format!("同步失败: {}", e));
            }
        }

        Ok(result)
    }

    async fn clear_pending_manual_sync(&self) {
        *self.pending_manual_sync.lock().await = None;
    }

    async fn store_pending_manual_sync(&self, pending: PendingManualSync) {
        *self.pending_manual_sync.lock().await = Some(pending);
    }

    async fn take_pending_manual_sync(&self) -> Option<PendingManualSync> {
        self.pending_manual_sync.lock().await.take()
    }

    fn default_incremental_sync_options() -> SyncOptions {
        SyncOptions {
            start_page: 1,
            max_pages: 100,
            bill_type: BillType::All,
            early_stop_threshold: 10,
        }
    }

    fn encode_captcha_image(image: &[u8]) -> String {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        BASE64.encode(image)
    }
}

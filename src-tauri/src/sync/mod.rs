use std::collections::VecDeque;

use chrono::{Duration, Local};
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

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRangePreset {
    Week,
    HalfMonth,
    Month,
    HalfYear,
    Year,
    All,
}

#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub account_id: String,
    pub current_account: String,
    pub account_index: usize,
    pub total_accounts: usize,
    pub new_count: usize,
    pub pages_fetched: u32,
    pub total_new_count: usize,
    pub status: SyncStatus,
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    ProbingLogin,
    GettingCaptcha,
    LoggingIn,
    Syncing { page: u32, total: u32 },
    Persisting,
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
    total_accounts: usize,
    sync_options: SyncOptions,
    current_account: Account,
    remaining_accounts: VecDeque<Account>,
    results: Vec<AccountSyncResult>,
    total_new_count: usize,
    epay: EpayAuth,
    execution: String,
}

#[derive(Debug, Clone)]
struct AccountProgressContext {
    account_id: String,
    account_name: String,
    account_index: usize,
    total_accounts: usize,
}

impl PendingManualSync {
    fn push_result(&mut self, result: AccountSyncResult) {
        self.total_new_count += result.new_count;
        self.results.push(result);
    }
}

impl AccountProgressContext {
    fn new(account: &Account, account_index: usize, total_accounts: usize) -> Self {
        Self {
            account_id: account.account_id.clone(),
            account_name: account.account_name.clone(),
            account_index,
            total_accounts,
        }
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

    fn remote_ocr_http_url(&self) -> String {
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok();
        let config = content.and_then(|c| toml::from_str::<crate::config::AppConfig>(&c).ok());
        config
            .map(|c| c.captcha.remote_ocr_http_url.clone())
            .unwrap_or_else(|| "http://127.0.0.1:5000".to_string())
    }

    fn ocr_retry_count(&self) -> usize {
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok();
        let config = content.and_then(|c| toml::from_str::<crate::config::AppConfig>(&c).ok());
        config.map(|c| c.captcha.ocr_retry_count).unwrap_or(3)
    }

    fn skip_graduated_accounts(&self) -> bool {
        let config_path = self.data_dir.join("app_config.toml");
        let content = std::fs::read_to_string(&config_path).ok();
        let config = content.and_then(|c| toml::from_str::<crate::config::AppConfig>(&c).ok());
        config
            .map(|c| c.sync.skip_graduated_accounts)
            .unwrap_or(true)
    }
}

impl BillSyncService {
    pub fn new(
        db_manager: DatabaseManager,
        crypto: CryptoService,
        translator: PositionTranslator,
    ) -> Self {
        Self {
            db_manager,
            crypto,
            pending_manual_sync: Mutex::new(None),
            translator,
        }
    }

    pub async fn get_enabled_accounts_for_identity(
        &self,
        identity_id: i64,
    ) -> AppResult<Vec<Account>> {
        let accounts = self
            .db_manager
            .list_accounts_by_identity(identity_id)
            .await?;
        let cfg = ConfigAccess::new(&self.db_manager);
        let skip_graduated_accounts = cfg.skip_graduated_accounts();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let enabled_accounts = accounts
            .into_iter()
            .filter(|a| {
                a.enable
                    && a.enable_update
                    && (!skip_graduated_accounts || !is_account_graduated(a, &today))
            })
            .collect::<Vec<_>>();
        tracing::info!(
            "[Sync] get_enabled_accounts_for_identity identity_id={} => {} enabled accounts",
            identity_id,
            enabled_accounts.len()
        );
        Ok(enabled_accounts)
    }

    pub async fn sync_identity(
        &self,
        identity_id: i64,
        sync_range: SyncRangePreset,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!(
            "[Sync] sync_identity called, identity_id={}, sync_range={:?}",
            identity_id,
            sync_range
        );
        let cfg = ConfigAccess::new(&self.db_manager);
        let sync_options = Self::default_incremental_sync_options(sync_range);

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
        tracing::info!(
            "[Sync] do_sync identity_id={} with {} accounts, options=start_page={}, max_pages={}, early_stop_threshold={}",
            identity_id,
            accounts.len(),
            sync_options.start_page,
            sync_options.max_pages,
            sync_options.early_stop_threshold
        );
        self.run_accounts(&accounts, sync_options, progress_callback)
            .await
    }

    async fn sync_single_account(
        &self,
        account: &Account,
        sync_options: &SyncOptions,
        progress_callback: Option<&SyncProgressCallback>,
        progress_context: &AccountProgressContext,
        total_new_before: usize,
    ) -> AppResult<AccountSyncResult> {
        tracing::info!(
            "[Sync] sync_single_account account_id={}, account_name={}, total_new_before={}",
            account.account_id,
            account.account_name,
            total_new_before
        );

        if let Some(cached_result) = self
            .try_sync_with_saved_session(
                account,
                sync_options,
                progress_callback,
                progress_context,
                total_new_before,
            )
            .await?
        {
            return Ok(cached_result);
        }

        let cfg = ConfigAccess::new(&self.db_manager);
        if matches!(cfg.captcha_mode(), crate::config::CaptchaMode::Manual) {
            tracing::info!(
                "[Sync] manual captcha mode for account_id={}, probing fresh login state",
                account.account_id
            );
            let mut epay = EpayAuth::new()?;
            match epay.probe_login().await? {
                LoginProbe::AlreadyLoggedIn => {
                    tracing::info!(
                        "[Sync] account_id={} already logged in without re-entering credentials",
                        account.account_id
                    );
                    Self::emit_progress(
                        progress_callback,
                        progress_context,
                        0,
                        0,
                        total_new_before,
                        SyncStatus::LoggingIn,
                    );
                    return self
                        .sync_logged_in_account(
                            account,
                            &epay,
                            sync_options,
                            progress_callback,
                            progress_context,
                            total_new_before,
                        )
                        .await;
                }
                LoginProbe::NeedLogin { .. } => {
                    tracing::info!(
                        "[Sync] account_id={} needs manual captcha login, storing pending state",
                        account.account_id
                    );
                    let challenge = epay.prepare_challenge().await?;
                    let image = Self::encode_captcha_image(&challenge.captcha_image);
                    let execution = challenge.execution.clone();

                    self.store_pending_manual_sync(PendingManualSync {
                        identity_id: account.identity_id,
                        total_accounts: 1,
                        sync_options: sync_options.clone(),
                        current_account: account.clone(),
                        remaining_accounts: VecDeque::new(),
                        results: Vec::new(),
                        total_new_count: 0,
                        epay,
                        execution: execution.clone(),
                    })
                    .await;

                    Self::emit_progress(
                        progress_callback,
                        progress_context,
                        0,
                        0,
                        total_new_before,
                        SyncStatus::GettingCaptcha,
                    );

                    return Err(AppError::Sync(format!(
                        "MANUAL_CAPTCHA_REQUIRED|{}|{}",
                        image, execution
                    )));
                }
            }
        }

        let password = self
            .db_manager
            .decrypt_account_password(account, &self.crypto)?;
        tracing::info!(
            "[Sync] account_id={} will use automatic login flow",
            account.account_id
        );
        let epay = self.login_auto(&account.account_id, &password).await?;
        self.save_session(&epay, &account.account_id).await?;
        Self::emit_progress(
            progress_callback,
            progress_context,
            0,
            0,
            total_new_before,
            SyncStatus::LoggingIn,
        );

        self.sync_logged_in_account(
            account,
            &epay,
            sync_options,
            progress_callback,
            progress_context,
            total_new_before,
        )
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
        tracing::info!(
            "[Sync] sync_identity_manual identity_id={} with {} accounts, options=start_page={}, max_pages={}, early_stop_threshold={}",
            identity_id,
            total_accounts,
            sync_options.start_page,
            sync_options.max_pages,
            sync_options.early_stop_threshold
        );
        let mut remaining_accounts: VecDeque<Account> = accounts.into();
        let mut results = Vec::new();
        let mut total_new_count = 0usize;
        let mut processed_accounts = 0usize;

        while let Some(account) = remaining_accounts.pop_front() {
            let progress_context =
                AccountProgressContext::new(&account, processed_accounts, total_accounts);
            Self::emit_progress(
                progress_callback,
                &progress_context,
                0,
                0,
                total_new_count,
                SyncStatus::ProbingLogin,
            );

            if let Some(result) = self
                .try_sync_with_saved_session(
                    &account,
                    &sync_options,
                    progress_callback,
                    &progress_context,
                    total_new_count,
                )
                .await?
            {
                tracing::info!(
                    "[Sync] manual flow reused saved session for account_id={}, new_count={}",
                    account.account_id,
                    result.new_count
                );
                total_new_count += result.new_count;
                Self::emit_progress(
                    progress_callback,
                    &progress_context,
                    result.new_count,
                    result.pages_fetched,
                    total_new_count,
                    SyncStatus::Completed,
                );
                results.push(result);
                processed_accounts += 1;
                continue;
            }

            let mut epay = EpayAuth::new()?;
            match epay.probe_login().await? {
                LoginProbe::AlreadyLoggedIn => {
                    tracing::info!(
                        "[Sync] manual flow probe indicates already logged in for account_id={}",
                        account.account_id
                    );
                    Self::emit_progress(
                        progress_callback,
                        &progress_context,
                        0,
                        0,
                        total_new_count,
                        SyncStatus::LoggingIn,
                    );
                    let result = self
                        .sync_logged_in_account(
                            &account,
                            &epay,
                            &sync_options,
                            progress_callback,
                            &progress_context,
                            total_new_count,
                        )
                        .await?;
                    total_new_count += result.new_count;
                    Self::emit_progress(
                        progress_callback,
                        &progress_context,
                        result.new_count,
                        result.pages_fetched,
                        total_new_count,
                        SyncStatus::Completed,
                    );
                    results.push(result);
                    processed_accounts += 1;
                }
                LoginProbe::NeedLogin { .. } => {
                    tracing::info!(
                        "[Sync] manual flow requires captcha for account_id={}, processed_accounts={}, remaining_accounts={}",
                        account.account_id,
                        processed_accounts,
                        remaining_accounts.len()
                    );
                    let challenge = epay.prepare_challenge().await?;
                    let image = Self::encode_captcha_image(&challenge.captcha_image);
                    let execution = challenge.execution.clone();

                    self.store_pending_manual_sync(PendingManualSync {
                        identity_id,
                        total_accounts,
                        sync_options,
                        current_account: account,
                        remaining_accounts,
                        results,
                        total_new_count,
                        epay,
                        execution: execution.clone(),
                    })
                    .await;

                    Self::emit_progress(
                        progress_callback,
                        &progress_context,
                        0,
                        0,
                        total_new_count,
                        SyncStatus::GettingCaptcha,
                    );

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
        let max_attempts = cfg.ocr_retry_count().max(1);

        for attempt in 1..=max_attempts {
            tracing::info!("[Sync] Login attempt {}/{}", attempt, max_attempts);
            let mut epay = EpayAuth::new()?;
            epay.probe_login().await?;
            let challenge = epay.prepare_challenge().await?;

            let captcha_code = match cfg.captcha_mode() {
                crate::config::CaptchaMode::RemoteOcr => {
                    let host = cfg.remote_ocr_host();
                    let port = cfg.remote_ocr_port();
                    if host.is_empty() || port == 0 {
                        return Err(AppError::Sync("未配置远程OCR服务器".to_string()));
                    }
                    tracing::info!("[Sync] Using remote OCR (TCP) {}:{}", host, port);
                    shmtu_cas::captcha::OcrCaptchaResolver::new(&host, port)
                        .with_retries(max_attempts)
                        .resolve(&challenge.captcha_image)
                        .await?
                        .into_final_answer()
                }
                crate::config::CaptchaMode::RemoteOcrHttp => {
                    let http_url = cfg.remote_ocr_http_url();
                    if http_url.is_empty() {
                        return Err(AppError::Sync("未配置RESTful OCR服务器地址".to_string()));
                    }
                    tracing::info!("[Sync] Using remote OCR (RESTful) {}", http_url);
                    shmtu_cas::captcha::OcrHttpCaptchaResolver::new(&http_url)
                        .with_retries(max_attempts)
                        .resolve(&challenge.captcha_image)
                        .await?
                        .into_final_answer()
                }
                _ => {
                    return Err(AppError::Sync("当前验证码模式不支持自动登录".to_string()));
                }
            };

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
                    tracing::warn!("[Sync] Captcha wrong, retry {}/{}", attempt, max_attempts);
                    if attempt < max_attempts {
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
        progress_callback: Option<&SyncProgressCallback>,
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
            tracing::warn!(
                "[Sync] pending captcha identity mismatch: pending_identity_id={}, requested_identity_id={}",
                pending.identity_id,
                identity_id
            );
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
                let progress_context = AccountProgressContext::new(
                    &pending.current_account,
                    pending.results.len(),
                    pending.total_accounts,
                );
                self.save_session(&pending.epay, &pending.current_account.account_id)
                    .await?;
                Self::emit_progress(
                    progress_callback,
                    &progress_context,
                    0,
                    0,
                    pending.total_new_count,
                    SyncStatus::LoggingIn,
                );
                let result = self
                    .sync_logged_in_account(
                        &pending.current_account,
                        &pending.epay,
                        &pending.sync_options,
                        progress_callback,
                        &progress_context,
                        pending.total_new_count,
                    )
                    .await?;
                pending.push_result(result);
                if let Some(last) = pending.results.last() {
                    Self::emit_progress(
                        progress_callback,
                        &progress_context,
                        last.new_count,
                        last.pages_fetched,
                        pending.total_new_count,
                        SyncStatus::Completed,
                    );
                }
                self.continue_pending_manual_sync(pending, progress_callback)
                    .await
            }
            LoginSubmitResult::ValidateCodeError => {
                tracing::error!("[Sync] Captcha WRONG!");
                let challenge = pending.epay.prepare_challenge().await?;
                let image = Self::encode_captcha_image(&challenge.captcha_image);
                pending.execution = challenge.execution.clone();
                let execution = pending.execution.clone();
                let progress_context = AccountProgressContext::new(
                    &pending.current_account,
                    pending.results.len(),
                    pending.total_accounts,
                );
                let total_new_count = pending.total_new_count;
                self.store_pending_manual_sync(pending).await;
                Self::emit_progress(
                    progress_callback,
                    &progress_context,
                    0,
                    0,
                    total_new_count,
                    SyncStatus::GettingCaptcha,
                );
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
            tracing::info!(
                "[Sync] refresh captcha from pending manual sync account_id={}, remaining_accounts={}",
                pending.current_account.account_id,
                pending.remaining_accounts.len()
            );
            let challenge = pending.epay.prepare_challenge().await?;
            let image = Self::encode_captcha_image(&challenge.captcha_image);
            pending.execution = challenge.execution.clone();
            let execution = pending.execution.clone();
            self.store_pending_manual_sync(pending).await;
            return Ok((image, execution));
        }

        let mut epay = EpayAuth::new()?;
        tracing::info!("[Sync] refresh captcha from fresh epay session");
        epay.probe_login().await?;
        let challenge = epay.prepare_challenge().await?;
        Ok((
            Self::encode_captcha_image(&challenge.captcha_image),
            challenge.execution,
        ))
    }

    async fn save_session(&self, epay: &EpayAuth, account_id: &str) -> AppResult<()> {
        let cookies_json = epay.extract_session()?;
        self.db_manager
            .save_session(account_id, &cookies_json, &self.crypto)
            .await?;
        tracing::info!(
            "[Sync] saved encrypted session for account_id={}, cookies_len={}",
            account_id,
            cookies_json.len()
        );
        Ok(())
    }

    pub async fn full_sync_identity(
        &self,
        identity_id: i64,
        sync_range: SyncRangePreset,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!(
            "[Sync] full_sync_identity, identity_id={}, sync_range={:?}",
            identity_id,
            sync_range
        );
        // 全量更新：清空旧数据
        self.db_manager.clear_merged_non_manual(identity_id).await?;
        let _ = self
            .db_manager
            .clear_operation_logs(identity_id, None)
            .await;
        let accounts = self.get_enabled_accounts_for_identity(identity_id).await?;
        // 清除所有账号的旧数据和 session
        for account in &accounts {
            let _ = self
                .db_manager
                .clear_account_original(&account.account_id)
                .await;
            let _ = self
                .db_manager
                .invalidate_session(&account.account_id)
                .await;
        }

        let sync_options = SyncOptions {
            start_page: 1,
            max_pages: 1000,
            bill_type: BillType::All,
            early_stop_threshold: u32::MAX,
            since_timestamp: Self::range_since_timestamp(sync_range),
        };

        let cfg = ConfigAccess::new(&self.db_manager);
        if matches!(cfg.captcha_mode(), crate::config::CaptchaMode::Manual) {
            tracing::info!("[Sync] full_sync_identity: manual captcha mode");
            self.clear_pending_manual_sync().await;
            return self
                .sync_identity_manual(identity_id, sync_options, progress_callback)
                .await;
        }

        self.run_accounts(&accounts, &sync_options, progress_callback)
            .await
    }

    /// 增量同步单个账号
    pub async fn sync_single_account_by_id(
        &self,
        identity_id: i64,
        account_id: &str,
        sync_range: SyncRangePreset,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!(
            "[Sync] sync_single_account_by_id, identity_id={}, account_id={}, sync_range={:?}",
            identity_id,
            account_id,
            sync_range
        );
        let account = self.find_enabled_account(identity_id, account_id).await?;

        let sync_options = Self::default_incremental_sync_options(sync_range);
        self.run_accounts(&[account], &sync_options, progress_callback)
            .await
    }

    /// 全量同步单个账号（清空旧数据 + 清空旧 session 后重新同步）
    pub async fn full_sync_single_account(
        &self,
        identity_id: i64,
        account_id: &str,
        sync_range: SyncRangePreset,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        tracing::info!(
            "[Sync] full_sync_single_account, identity_id={}, account_id={}, sync_range={:?}",
            identity_id,
            account_id,
            sync_range
        );
        let account = self.find_enabled_account(identity_id, account_id).await?;

        // 全量更新：清除该账号的旧数据和 session
        let _ = self
            .db_manager
            .clear_account_original(&account.account_id)
            .await;
        // 清除旧 session，强制重新登录
        let _ = self
            .db_manager
            .invalidate_session(&account.account_id)
            .await;
        // 清除该账号在合并表中的相关记录（非手动）
        let _ = self
            .db_manager
            .clear_merged_by_account(identity_id, &account.account_id)
            .await;

        let sync_options = SyncOptions {
            start_page: 1,
            max_pages: 1000,
            bill_type: BillType::All,
            early_stop_threshold: u32::MAX,
            since_timestamp: Self::range_since_timestamp(sync_range),
        };
        self.run_accounts(&[account], &sync_options, progress_callback)
            .await
    }

    async fn continue_pending_manual_sync(
        &self,
        mut pending: PendingManualSync,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        while let Some(account) = pending.remaining_accounts.pop_front() {
            tracing::info!(
                "[Sync] continue_pending_manual_sync processing account_id={}, completed_accounts={}, remaining_after_pop={}",
                account.account_id,
                pending.results.len(),
                pending.remaining_accounts.len()
            );
            let progress_context = AccountProgressContext::new(
                &account,
                pending.results.len(),
                pending.total_accounts,
            );
            Self::emit_progress(
                progress_callback,
                &progress_context,
                0,
                0,
                pending.total_new_count,
                SyncStatus::ProbingLogin,
            );

            if let Some(result) = self
                .try_sync_with_saved_session(
                    &account,
                    &pending.sync_options,
                    progress_callback,
                    &progress_context,
                    pending.total_new_count,
                )
                .await?
            {
                tracing::info!(
                    "[Sync] continue_pending_manual_sync reused saved session for account_id={}, total_new_count={}",
                    account.account_id,
                    pending.total_new_count
                );
                pending.push_result(result);
                if let Some(last) = pending.results.last() {
                    Self::emit_progress(
                        progress_callback,
                        &progress_context,
                        last.new_count,
                        last.pages_fetched,
                        pending.total_new_count,
                        SyncStatus::Completed,
                    );
                }
                continue;
            }

            let mut epay = EpayAuth::new()?;
            match epay.probe_login().await? {
                LoginProbe::AlreadyLoggedIn => {
                    tracing::info!(
                        "[Sync] continue_pending_manual_sync probe indicates already logged in for account_id={}",
                        account.account_id
                    );
                    Self::emit_progress(
                        progress_callback,
                        &progress_context,
                        0,
                        0,
                        pending.total_new_count,
                        SyncStatus::LoggingIn,
                    );
                    let result = self
                        .sync_logged_in_account(
                            &account,
                            &epay,
                            &pending.sync_options,
                            progress_callback,
                            &progress_context,
                            pending.total_new_count,
                        )
                        .await?;
                    pending.push_result(result);
                    if let Some(last) = pending.results.last() {
                        Self::emit_progress(
                            progress_callback,
                            &progress_context,
                            last.new_count,
                            last.pages_fetched,
                            pending.total_new_count,
                            SyncStatus::Completed,
                        );
                    }
                }
                LoginProbe::NeedLogin { .. } => {
                    tracing::info!(
                        "[Sync] continue_pending_manual_sync needs captcha for next account_id={}, remaining_accounts={}",
                        account.account_id,
                        pending.remaining_accounts.len()
                    );
                    let challenge = epay.prepare_challenge().await?;
                    let image = Self::encode_captcha_image(&challenge.captcha_image);
                    pending.current_account = account;
                    pending.epay = epay;
                    pending.execution = challenge.execution.clone();
                    let execution = pending.execution.clone();
                    let total_new_count = pending.total_new_count;
                    self.store_pending_manual_sync(pending).await;
                    Self::emit_progress(
                        progress_callback,
                        &progress_context,
                        0,
                        0,
                        total_new_count,
                        SyncStatus::GettingCaptcha,
                    );
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
        progress_callback: Option<&SyncProgressCallback>,
        progress_context: &AccountProgressContext,
        total_new_before: usize,
    ) -> AppResult<Option<AccountSyncResult>> {
        tracing::info!(
            "[Sync] try_sync_with_saved_session account_id={}, total_new_before={}",
            account.account_id,
            total_new_before
        );
        if let Some(session) = self
            .db_manager
            .get_session(&account.account_id, &self.crypto)
            .await?
        {
            tracing::info!(
                "[Sync] found saved session for account_id={}, expire_time={:?}",
                account.account_id,
                session.expire_time
            );
            let mut epay = EpayAuth::new()?;
            epay.restore_session(&session.cookies)?;
            if let Ok(LoginProbe::AlreadyLoggedIn) = epay.probe_login().await {
                tracing::info!("[Sync] Session valid for {}", account.account_id);
                Self::emit_progress(
                    progress_callback,
                    progress_context,
                    0,
                    0,
                    total_new_before,
                    SyncStatus::LoggingIn,
                );
                return Ok(Some(
                    self.sync_logged_in_account(
                        account,
                        &epay,
                        sync_options,
                        progress_callback,
                        progress_context,
                        total_new_before,
                    )
                    .await?,
                ));
            }
            tracing::info!(
                "[Sync] saved session exists but is no longer valid for account_id={}",
                account.account_id
            );
        } else {
            tracing::info!(
                "[Sync] no saved session available for account_id={}",
                account.account_id
            );
        }

        Ok(None)
    }

    async fn run_accounts(
        &self,
        accounts: &[Account],
        sync_options: &SyncOptions,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        let account_labels = accounts
            .iter()
            .map(|a| format!("{}({})", a.account_name, a.account_id))
            .collect::<Vec<_>>()
            .join(", ");
        tracing::info!(
            "[Sync] run_accounts start total_accounts={}, accounts=[{}]",
            accounts.len(),
            account_labels
        );
        let mut results = Vec::with_capacity(accounts.len());
        let mut total_new_count = 0usize;

        for (account_index, account) in accounts.iter().enumerate() {
            let account_result = self
                .run_account_with_progress(
                    account,
                    sync_options,
                    progress_callback,
                    account_index,
                    accounts.len(),
                    total_new_count,
                )
                .await?;
            total_new_count += account_result.new_count;
            results.push(account_result);
        }

        tracing::info!(
            "[Sync] run_accounts completed total_accounts={}, total_new_count={}",
            accounts.len(),
            total_new_count
        );

        Ok(IdentitySyncResult {
            results,
            total_new_count,
        })
    }

    async fn run_account_with_progress(
        &self,
        account: &Account,
        sync_options: &SyncOptions,
        progress_callback: Option<&SyncProgressCallback>,
        account_index: usize,
        total_accounts: usize,
        total_new_before: usize,
    ) -> AppResult<AccountSyncResult> {
        tracing::info!(
            "[Sync] run_account_with_progress account_id={}, account_index={}/{}, total_new_before={}",
            account.account_id,
            account_index + 1,
            total_accounts,
            total_new_before
        );
        let progress_context = AccountProgressContext::new(account, account_index, total_accounts);
        Self::emit_progress(
            progress_callback,
            &progress_context,
            0,
            0,
            total_new_before,
            SyncStatus::ProbingLogin,
        );

        let account_result = self
            .sync_single_account(
                account,
                sync_options,
                progress_callback,
                &progress_context,
                total_new_before,
            )
            .await?;

        Self::emit_progress(
            progress_callback,
            &progress_context,
            account_result.new_count,
            account_result.pages_fetched,
            total_new_before + account_result.new_count,
            SyncStatus::Completed,
        );

        Ok(account_result)
    }

    async fn find_enabled_account(&self, identity_id: i64, account_id: &str) -> AppResult<Account> {
        tracing::info!(
            "[Sync] find_enabled_account identity_id={}, account_id={}",
            identity_id,
            account_id
        );
        let accounts = self.get_enabled_accounts_for_identity(identity_id).await?;
        let result = accounts
            .into_iter()
            .find(|a| a.account_id == account_id)
            .ok_or_else(|| AppError::Sync(format!("账号 {} 不存在或已禁用", account_id)));
        if result.is_ok() {
            tracing::info!(
                "[Sync] find_enabled_account matched identity_id={}, account_id={}",
                identity_id,
                account_id
            );
        } else {
            tracing::warn!(
                "[Sync] find_enabled_account missing or disabled identity_id={}, account_id={}",
                identity_id,
                account_id
            );
        }
        result
    }

    async fn sync_logged_in_account(
        &self,
        account: &Account,
        epay: &EpayAuth,
        sync_options: &SyncOptions,
        progress_callback: Option<&SyncProgressCallback>,
        progress_context: &AccountProgressContext,
        total_new_before: usize,
    ) -> AppResult<AccountSyncResult> {
        let mut store = BillStoreImpl::new(
            self.db_manager.db().clone(),
            &account.account_id,
            account.identity_id,
            self.translator.clone(),
        )
        .await?;
        let mut result = AccountSyncResult {
            account_id: account.account_id.clone(),
            account_name: account.account_name.clone(),
            new_count: 0,
            pages_fetched: 0,
            early_stopped: false,
            error: None,
        };

        tracing::info!(
            "[Sync] begin fetching bills for account={}, identity_id={}",
            account.account_id,
            account.identity_id
        );
        let page_progress_callback = |page_progress: shmtu_cas::sync::SyncPageProgress| {
            Self::emit_progress(
                progress_callback,
                progress_context,
                page_progress.new_count,
                page_progress.page,
                total_new_before + page_progress.new_count,
                SyncStatus::Syncing {
                    page: page_progress.page,
                    total: page_progress.total_pages,
                },
            );
        };
        match shmtu_cas::sync::incremental_sync_with_progress(
            epay,
            &mut store,
            sync_options,
            progress_callback.map(|_| &page_progress_callback),
        )
        .await
        {
            Ok(sync_result) => {
                tracing::info!(
                    "[Sync] fetched bills for account={}, pages_fetched={}, new_count={}, early_stopped={}, flushing_to_db=true",
                    account.account_id,
                    sync_result.pages_fetched,
                    sync_result.new_count,
                    sync_result.early_stopped
                );
                Self::emit_progress(
                    progress_callback,
                    progress_context,
                    sync_result.new_count,
                    sync_result.pages_fetched,
                    total_new_before + sync_result.new_count,
                    SyncStatus::Persisting,
                );
                store.flush_pending_bills().await?;
                tracing::info!(
                    "[Sync] flush complete for account={}, merged_into_identity_id={}",
                    account.account_id,
                    account.identity_id
                );
                result.new_count = sync_result.new_count;
                result.pages_fetched = sync_result.pages_fetched;
                result.early_stopped = sync_result.early_stopped;
                let _ = self.db_manager.update_account_last_sync(account.id).await;
            }
            Err(e) => {
                tracing::error!(
                    "[Sync] incremental_sync_with_progress failed for account_id={}: {}",
                    account.account_id,
                    e
                );
                result.error = Some(format!("同步失败: {}", e));
            }
        }

        Ok(result)
    }

    fn emit_progress(
        progress_callback: Option<&SyncProgressCallback>,
        progress_context: &AccountProgressContext,
        new_count: usize,
        pages_fetched: u32,
        total_new_count: usize,
        status: SyncStatus,
    ) {
        let progress_message = match &status {
            SyncStatus::ProbingLogin => format!(
                "账号 {} 正在检查登录状态（{}/{}）",
                progress_context.account_name,
                progress_context.account_index + 1,
                progress_context.total_accounts
            ),
            SyncStatus::GettingCaptcha => format!(
                "账号 {} 需要验证码（{}/{}），累计新增 {} 条",
                progress_context.account_name,
                progress_context.account_index + 1,
                progress_context.total_accounts,
                total_new_count
            ),
            SyncStatus::LoggingIn => format!(
                "账号 {} 已通过登录检查，正在准备拉取账单（{}/{}），累计新增 {} 条",
                progress_context.account_name,
                progress_context.account_index + 1,
                progress_context.total_accounts,
                total_new_count
            ),
            SyncStatus::Syncing { page, total } => format!(
                "账号 {} 正在从校园平台拉取账单第 {}/{} 页，当前账号新增 {} 条，累计新增 {} 条（{}/{}）",
                progress_context.account_name,
                page,
                total,
                new_count,
                total_new_count,
                progress_context.account_index + 1,
                progress_context.total_accounts
            ),
            SyncStatus::Persisting => format!(
                "账号 {} 已拉取完成，正在写入原始账单并合并到身份：新增 {} 条，拉取 {} 页，累计新增 {} 条（{}/{}）",
                progress_context.account_name,
                new_count,
                pages_fetched,
                total_new_count,
                progress_context.account_index + 1,
                progress_context.total_accounts
            ),
            SyncStatus::Completed => format!(
                "账号 {} 拉取完成并已写入原始账单、合并到身份：新增 {} 条，拉取 {} 页，累计新增 {} 条（{}/{}）",
                progress_context.account_name,
                new_count,
                pages_fetched,
                total_new_count,
                progress_context.account_index + 1,
                progress_context.total_accounts
            ),
            SyncStatus::Failed(err) => format!(
                "账号 {} 同步失败（{}/{}）：{}",
                progress_context.account_name,
                progress_context.account_index + 1,
                progress_context.total_accounts,
                err
            ),
        };
        tracing::info!("[SyncProgress] {}", progress_message);

        if let Some(cb) = progress_callback {
            cb(SyncProgress {
                account_id: progress_context.account_id.clone(),
                current_account: progress_context.account_name.clone(),
                account_index: progress_context.account_index,
                total_accounts: progress_context.total_accounts,
                new_count,
                pages_fetched,
                total_new_count,
                status,
            });
        }
    }

    async fn clear_pending_manual_sync(&self) {
        tracing::info!("[Sync] clear_pending_manual_sync");
        *self.pending_manual_sync.lock().await = None;
    }

    async fn store_pending_manual_sync(&self, pending: PendingManualSync) {
        tracing::info!(
            "[Sync] store_pending_manual_sync identity_id={}, current_account_id={}, total_accounts={}, completed_accounts={}, remaining_accounts={}",
            pending.identity_id,
            pending.current_account.account_id,
            pending.total_accounts,
            pending.results.len(),
            pending.remaining_accounts.len()
        );
        *self.pending_manual_sync.lock().await = Some(pending);
    }

    async fn take_pending_manual_sync(&self) -> Option<PendingManualSync> {
        let pending = self.pending_manual_sync.lock().await.take();
        match &pending {
            Some(p) => tracing::info!(
                "[Sync] take_pending_manual_sync hit identity_id={}, current_account_id={}, remaining_accounts={}",
                p.identity_id,
                p.current_account.account_id,
                p.remaining_accounts.len()
            ),
            None => tracing::info!("[Sync] take_pending_manual_sync miss"),
        }
        pending
    }

    fn default_incremental_sync_options(sync_range: SyncRangePreset) -> SyncOptions {
        SyncOptions {
            start_page: 1,
            max_pages: 100,
            bill_type: BillType::All,
            early_stop_threshold: 10,
            since_timestamp: Self::range_since_timestamp(sync_range),
        }
    }

    fn range_since_timestamp(sync_range: SyncRangePreset) -> Option<i64> {
        let now = Local::now();
        let since = match sync_range {
            SyncRangePreset::Week => Some(now - Duration::days(7)),
            SyncRangePreset::HalfMonth => Some(now - Duration::days(15)),
            SyncRangePreset::Month => Some(now - Duration::days(30)),
            SyncRangePreset::HalfYear => Some(now - Duration::days(183)),
            SyncRangePreset::Year => Some(now - Duration::days(365)),
            SyncRangePreset::All => None,
        };
        since.map(|dt| dt.timestamp())
    }

    fn encode_captcha_image(image: &[u8]) -> String {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        BASE64.encode(image)
    }
}

fn is_account_graduated(account: &Account, today: &str) -> bool {
    match account.graduation_date.as_deref().map(str::trim) {
        Some("") | None => false,
        Some(graduation_date) => graduation_date < today,
    }
}

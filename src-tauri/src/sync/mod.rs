use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use shmtu_cas::datatype::bill::BillType;
use shmtu_cas::sync::{incremental_sync, SyncOptions};

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

/// 账单同步服务
pub struct BillSyncService {
    db_manager: DatabaseManager,
    crypto: CryptoService,
}

impl BillSyncService {
    /// 创建同步服务
    pub fn new(db_manager: DatabaseManager, crypto: CryptoService) -> Self {
        Self { db_manager, crypto }
    }

    /// 获取身份下所有启用的账号
    fn get_enabled_accounts(&self, identity_id: i64) -> AppResult<Vec<Account>> {
        let accounts = self.db_manager.list_accounts_by_identity(identity_id)?;
        Ok(accounts.into_iter().filter(|a| a.enable && a.enable_update).collect())
    }

    /// 单账号增量同步
    pub async fn sync_account(
        &self,
        account: &Account,
        sync_options: &SyncOptions,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<AccountSyncResult> {
        let mut result = AccountSyncResult {
            account_id: account.account_id.clone(),
            account_name: account.account_name.clone(),
            new_count: 0,
            pages_fetched: 0,
            early_stopped: false,
            error: None,
        };

        // 获取身份信息
        let _identity = match self.db_manager.get_identity(account.identity_id)? {
            Some(id) => id,
            None => {
                result.error = Some("身份不存在".to_string());
                return Ok(result);
            }
        };

        // 创建 BillStore
        let mut store = match BillStoreImpl::new(
            self.db_manager.clone_ref(),
            &account.account_id,
            account.identity_id,
        ) {
            Ok(s) => s,
            Err(e) => {
                result.error = Some(format!("创建存储失败: {}", e));
                return Ok(result);
            }
        };

        // 创建 EpayAuth 并登录
        let epay = match self.login_epay(account, progress_callback).await {
            Ok(e) => e,
            Err(e) => {
                result.error = Some(format!("登录失败: {}", e));
                return Ok(result);
            }
        };

        // 执行增量同步
        match incremental_sync(&epay, &mut store, sync_options).await {
            Ok(sync_result) => {
                result.new_count = sync_result.new_count;
                result.pages_fetched = sync_result.pages_fetched;
                result.early_stopped = sync_result.early_stopped;

                // 更新最后同步时间
                let _ = self.db_manager.update_account_last_sync(account.id);
            }
            Err(e) => {
                result.error = Some(format!("同步失败: {}", e));
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
        let accounts = self.get_enabled_accounts(identity_id)?;
        let total = accounts.len();
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

            let account_result = self.sync_account(account, sync_options, progress_callback).await?;
            total_new += account_result.new_count;
            results.push(account_result);
        }

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
        // 清空账号原始数据
        self.db_manager.clear_account_original(&account.account_id)?;
        // 清空该账号来源的合并数据
        self.db_manager.clear_merged_by_account(account.identity_id, &account.account_id)?;
        // 清空该账号的操作日志
        self.db_manager.clear_operation_logs(account.identity_id, Some(&account.account_id))?;

        // 全量同步选项：不提前停止
        let sync_options = SyncOptions {
            start_page: 1,
            max_pages: 1000,
            bill_type: BillType::All,
            early_stop_threshold: u32::MAX,
        };

        self.sync_account(account, &sync_options, progress_callback).await
    }

    /// 身份级别全量更新
    pub async fn full_sync_identity(
        &self,
        identity_id: i64,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        let accounts = self.get_enabled_accounts(identity_id)?;

        // 清空身份的合并数据（非手动）
        self.db_manager.clear_merged_non_manual(identity_id)?;
        // 清空所有操作日志
        let _ = self.db_manager.clear_operation_logs(identity_id, None);

        let total = accounts.len();
        let mut results = Vec::new();
        let mut total_new = 0;

        for (idx, account) in accounts.iter().enumerate() {
            // 清空账号原始数据
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

            let account_result = self.sync_account(account, &sync_options, progress_callback).await?;
            total_new += account_result.new_count;
            results.push(account_result);
        }

        Ok(IdentitySyncResult {
            results,
            total_new_count: total_new,
        })
    }

    /// CAS 登录流程
    async fn login_epay(
        &self,
        account: &Account,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<EpayAuth> {
        let mut epay = EpayAuth::new()?;

        // 解密密码
        let _password = self.db_manager.decrypt_account_password(account, &self.crypto)?;

        // 尝试探测登录状态
        if let Some(cb) = progress_callback {
            cb(SyncProgress {
                current_account: account.account_name.clone(),
                account_index: 0,
                total_accounts: 1,
                status: SyncStatus::ProbingLogin,
            });
        }

        match epay.probe_login().await? {
            LoginProbe::AlreadyLoggedIn => {
                // 已登录，直接返回
                return Ok(epay);
            }
            LoginProbe::NeedLogin { .. } => {}
        }

        // 需要登录，获取验证码
        if let Some(cb) = progress_callback {
            cb(SyncProgress {
                current_account: account.account_name.clone(),
                account_index: 0,
                total_accounts: 1,
                status: SyncStatus::GettingCaptcha,
            });
        }

        let _challenge = epay.prepare_challenge().await?;

        // TODO: 这里需要根据验证码模式来处理
        // 当前先抛出错误，需要前端配合处理验证码
        // 实际使用时，验证码图片会发送到前端，用户输入后回调
        // 此处留出接口，后续由 Tauri Command 层实现验证码回调
        Err(AppError::Sync(
            "需要验证码，请通过前端交互完成登录".to_string(),
        ))
    }

    /// 使用验证码完成登录（前端回调后调用）
    pub async fn login_with_captcha(
        &self,
        account: &Account,
        validate_code: &str,
    ) -> AppResult<EpayAuth> {
        let mut epay = EpayAuth::new()?;
        let password = self.db_manager.decrypt_account_password(account, &self.crypto)?;

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
                // 保存会话
                // TODO: 提取 cookies 并保存
                Ok(epay)
            }
            LoginSubmitResult::ValidateCodeError => {
                Err(AppError::Sync("验证码错误".to_string()))
            }
            LoginSubmitResult::PasswordError => {
                Err(AppError::Sync("密码错误".to_string()))
            }
            LoginSubmitResult::Failure(msg) => {
                Err(AppError::Sync(format!("登录失败: {}", msg)))
            }
        }
    }
}

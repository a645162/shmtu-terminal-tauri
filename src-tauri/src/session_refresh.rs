use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::RwLock;
use tokio::time::{interval, Instant};

use shmtu_cas::cas::epay::{EpayAuth, LoginProbe};

use crate::config::TomlConfig;
use crate::crypto::CryptoService;
use crate::db::DatabaseManager;

/// Session 过期检查状态
pub struct SessionExpirationState {
    pub is_running: bool,
    pub last_check: Option<Instant>,
    pub total_checks: u64,
    pub valid_count: u64,
    pub expired_count: u64,
}

/// Session 过期检查服务
pub struct SessionExpirationService {
    config: Arc<RwLock<TomlConfig>>,
    db_manager: Arc<DatabaseManager>,
    crypto: Arc<CryptoService>,
    state: Arc<RwLock<SessionExpirationState>>,
}

impl SessionExpirationService {
    pub fn new(
        config: Arc<RwLock<TomlConfig>>,
        db_manager: Arc<DatabaseManager>,
        crypto: Arc<CryptoService>,
    ) -> Self {
        Self {
            config,
            db_manager,
            crypto,
            state: Arc::new(RwLock::new(SessionExpirationState {
                is_running: false,
                last_check: None,
                total_checks: 0,
                valid_count: 0,
                expired_count: 0,
            })),
        }
    }

    /// 启动 session 过期检查服务
    pub async fn start(&self) {
        let mut state = self.state.write().await;
        if state.is_running {
            tracing::info!("[SessionExpiration] 服务已在运行中");
            return;
        }
        state.is_running = true;
        drop(state);

        // 获取配置
        let check_interval = {
            let config = self.config.read().await;
            let session_cfg = &config.get().session;
            if !session_cfg.auto_refresh {
                tracing::info!("[SessionExpiration] 自动检查已禁用");
                let mut state = self.state.write().await;
                state.is_running = false;
                return;
            }
            session_cfg.refresh_interval_minutes
        };

        tracing::info!("[SessionExpiration] 启动成功 | Interval={}分钟", check_interval);

        let config = self.config.clone();
        let db_manager = self.db_manager.clone();
        let crypto = self.crypto.clone();
        let state = self.state.clone();

        // 启动后台任务
        tokio::spawn(async move {
            let rng = rand_simple();
            let base_interval = check_interval * 60; // 转换为秒

            loop {
                // 随机浮动 ±1 分钟
                let jitter: i64 = ((rng % 3) as i64) - 1; // -1, 0, 或 1
                let jitter_seconds = (jitter * 60).max(0) as u64;
                let interval_seconds = (base_interval as i64 + jitter_seconds as i64).max(60) as u64;

                tracing::debug!("[SessionExpiration] 下次检查 | Interval={}秒", interval_seconds);

                let sleep_duration = Duration::from_secs(interval_seconds);

                // 使用 interval 实现定时
                let mut ticker = interval(sleep_duration);
                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                tokio::select! {
                    biased; // biased 让 stop 信号优先

                    _ = tokio::time::sleep(Duration::from_secs(3600)) => {
                        // 模拟停止信号（实际应该用 channel）
                        break;
                    }
                    _ = ticker.tick() => {
                        Self::perform_check(&config, &db_manager, &crypto, &state).await;
                    }
                }
            }

            let mut state = state.write().await;
            state.is_running = false;
        });
    }

    /// 停止服务
    pub async fn stop(&self) {
        let mut state = self.state.write().await;
        state.is_running = false;
        tracing::info!("[SessionExpiration] 已请求停止");
    }

    /// 重启服务
    pub async fn restart(&self) {
        self.stop().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        self.start().await;
    }

    /// 获取当前状态
    pub async fn get_status(&self) -> SessionExpirationStatus {
        let state = self.state.read().await;
        SessionExpirationStatus {
            is_running: state.is_running,
            last_check: state.last_check.map(|i| i.elapsed().as_secs()),
            total_checks: state.total_checks,
            valid_count: state.valid_count,
            expired_count: state.expired_count,
        }
    }

    /// 执行一次检查
    pub async fn check_now(&self) -> SessionExpirationResult {
        Self::perform_check(&self.config, &self.db_manager, &self.crypto, &self.state).await
    }

    async fn perform_check(
        config: &Arc<RwLock<TomlConfig>>,
        db_manager: &Arc<DatabaseManager>,
        crypto: &Arc<CryptoService>,
        state: &Arc<RwLock<SessionExpirationState>>,
    ) -> SessionExpirationResult {
        tracing::debug!("[SessionExpiration] 开始检查 session 状态");

        // 获取默认身份
        let identity_id = {
            let cfg = config.read().await;
            cfg.get().identity.default_identity_id
        };

        if identity_id == 0 {
            tracing::debug!("[SessionExpiration] 无默认身份，跳过检查");
            return SessionExpirationResult::default();
        }

        // 获取启用的账号
        let accounts = match db_manager.list_accounts_by_identity(identity_id).await {
            Ok(accounts) => accounts.into_iter().filter(|a| a.enable).collect::<Vec<_>>(),
            Err(e) => {
                tracing::error!("[SessionExpiration] 获取账号失败: {}", e);
                return SessionExpirationResult::default();
            }
        };

        if accounts.is_empty() {
            tracing::debug!("[SessionExpiration] 无启用的账号，跳过检查");
            return SessionExpirationResult::default();
        }

        let mut result = SessionExpirationResult::default();
        result.total_accounts = accounts.len();

        for account in accounts {
            let account_result = Self::check_and_invalidate_expired_session(&account, db_manager, crypto).await;
            let is_valid = account_result.is_valid;
            result.results.push(account_result);
            if is_valid {
                result.valid_count += 1;
            } else {
                result.expired_count += 1;
            }
        }

        // 更新状态
        {
            let mut state = state.write().await;
            state.last_check = Some(Instant::now());
            state.total_checks += 1;
            state.valid_count += result.valid_count as u64;
            state.expired_count += result.expired_count as u64;
        }

        tracing::info!(
            "[SessionExpiration] 检查完成 | Total={} | Valid={} | Expired={}",
            result.total_accounts,
            result.valid_count,
            result.expired_count
        );

        result
    }

    /// 检查并使过期的 session 失效
    async fn check_and_invalidate_expired_session(
        account: &crate::models::Account,
        db_manager: &Arc<DatabaseManager>,
        crypto: &Arc<CryptoService>,
    ) -> AccountExpirationResult {
        let mut result = AccountExpirationResult {
            account_id: account.account_id.clone(),
            ..Default::default()
        };

        // 1. 获取保存的 session
        let session = match db_manager.get_session(&account.account_id, crypto).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                result.is_valid = false;
                result.status = "no_session".to_string();
                tracing::debug!("[SessionExpiration] 无保存的 session | AccountId={}", account.account_id);
                return result;
            }
            Err(e) => {
                result.is_valid = false;
                result.status = "error".to_string();
                result.error_message = Some(e.to_string());
                tracing::error!("[SessionExpiration] 获取 session 失败: {}", e);
                return result;
            }
        };

        // 2. 创建 EpayAuth 并恢复 session
        let mut epay = match EpayAuth::new() {
            Ok(epay) => epay,
            Err(e) => {
                result.is_valid = false;
                result.status = "error".to_string();
                result.error_message = Some(e.to_string());
                return result;
            }
        };

        if let Err(e) = epay.restore_session(&session.cookies) {
            result.is_valid = false;
            result.status = "restore_failed".to_string();
            result.error_message = Some(e.to_string());
            tracing::warn!("[SessionExpiration] 恢复 session 失败: {}", e);
            return result;
        }

        // 3. 探测登录状态
        match epay.probe_login().await {
            Ok(LoginProbe::AlreadyLoggedIn) => {
                result.is_valid = true;
                result.status = "valid".to_string();
                tracing::debug!("[SessionExpiration] Session 有效 | AccountId={}", account.account_id);
            }
            Ok(LoginProbe::NeedLogin { .. }) => {
                // Session 已过期，标记为无效并停止后续检查
                tracing::info!("[SessionExpiration] 检测到过期 session，正在标记为无效 | AccountId={}", account.account_id);
                if let Err(e) = db_manager.invalidate_session(&account.account_id).await {
                    tracing::warn!("[SessionExpiration] 标记 session 失效失败: {}", e);
                }
                result.is_valid = false;
                result.status = "expired".to_string();
                result.was_invalidated = true;
            }
            Err(e) => {
                result.is_valid = false;
                result.status = "probe_error".to_string();
                result.error_message = Some(e.to_string());
                tracing::error!("[SessionExpiration] 探测登录状态失败: {}", e);
            }
        }

        result
    }
}

/// 简单的随机数生成器
fn rand_simple() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

/// Session 检查结果
#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionExpirationResult {
    pub total_accounts: usize,
    pub valid_count: usize,
    pub expired_count: usize,
    pub results: Vec<AccountExpirationResult>,
}

/// 单个账号的检查结果
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountExpirationResult {
    pub account_id: String,
    pub is_valid: bool,
    pub status: String,
    /// 是否已被标记为失效
    pub was_invalidated: bool,
    pub error_message: Option<String>,
}

/// Session 检查状态
#[derive(Debug, Clone, Serialize)]
pub struct SessionExpirationStatus {
    pub is_running: bool,
    pub last_check: Option<u64>, // 秒前
    pub total_checks: u64,
    pub valid_count: u64,
    pub expired_count: u64,
}

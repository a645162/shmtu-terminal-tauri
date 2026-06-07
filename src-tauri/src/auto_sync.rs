use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::Instant;

use crate::config::{CaptchaMode, SyncRangePresetConfig, TomlConfig};
use crate::session_refresh::rand_simple;
use crate::sync::{BillSyncService, SyncRangePreset};

pub struct AutoSyncState {
    pub is_running: bool,
    pub last_run: Option<Instant>,
    pub next_run: Option<Instant>,
    pub total_runs: u64,
    pub success_runs: u64,
    pub failed_runs: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AutoSyncStatus {
    pub is_running: bool,
    pub last_run_seconds_ago: Option<u64>,
    pub next_run_in_seconds: Option<u64>,
    pub total_runs: u64,
    pub success_runs: u64,
    pub failed_runs: u64,
}

pub struct AutoSyncService {
    config: Arc<RwLock<TomlConfig>>,
    sync_service: Arc<RwLock<BillSyncService>>,
    state: Arc<RwLock<AutoSyncState>>,
}

impl AutoSyncService {
    pub fn new(
        config: Arc<RwLock<TomlConfig>>,
        sync_service: Arc<RwLock<BillSyncService>>,
    ) -> Self {
        Self {
            config,
            sync_service,
            state: Arc::new(RwLock::new(AutoSyncState {
                is_running: false,
                last_run: None,
                next_run: None,
                total_runs: 0,
                success_runs: 0,
                failed_runs: 0,
            })),
        }
    }

    pub async fn start(&self) {
        let mut state = self.state.write().await;
        if state.is_running {
            tracing::info!("[AutoSync] 服务已在运行中");
            return;
        }
        state.is_running = true;
        drop(state);

        let (enabled, interval_minutes) = {
            let config = self.config.read().await;
            let sync_cfg = &config.get().sync;
            (
                sync_cfg.auto_sync_enabled,
                sync_cfg.auto_sync_interval_minutes.max(5),
            )
        };

        if !enabled {
            tracing::info!("[AutoSync] 自动同步已禁用");
            let mut state = self.state.write().await;
            state.is_running = false;
            return;
        }

        tracing::info!("[AutoSync] 启动成功 | Interval={}分钟", interval_minutes);

        let config = self.config.clone();
        let sync_service = self.sync_service.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            let rng = rand_simple();
            let base_interval = interval_minutes * 60;

            loop {
                let jitter: i64 = ((rng % 3) as i64) - 1;
                let jitter_seconds = (jitter * 60).max(0) as u64;
                let interval_seconds =
                    (base_interval as i64 + jitter_seconds as i64).max(300) as u64;
                let sleep_duration = Duration::from_secs(interval_seconds);
                {
                    let mut status = state.write().await;
                    status.next_run = Some(Instant::now() + sleep_duration);
                }
                let mut ticker = tokio::time::interval(sleep_duration);
                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                tokio::select! {
                    biased;
                    _ = tokio::time::sleep(Duration::from_secs(3600)) => {
                        break;
                    }
                    _ = ticker.tick() => {
                        Self::perform_sync(&config, &sync_service, &state).await;
                    }
                }
            }

            let mut state = state.write().await;
            state.is_running = false;
            state.next_run = None;
        });
    }

    pub async fn stop(&self) {
        let mut state = self.state.write().await;
        state.is_running = false;
        state.next_run = None;
        tracing::info!("[AutoSync] 已请求停止");
    }

    pub async fn restart(&self) {
        self.stop().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        self.start().await;
    }

    pub async fn get_status(&self) -> AutoSyncStatus {
        let state = self.state.read().await;
        AutoSyncStatus {
            is_running: state.is_running,
            last_run_seconds_ago: state.last_run.map(|i| i.elapsed().as_secs()),
            next_run_in_seconds: state
                .next_run
                .map(|i| i.saturating_duration_since(Instant::now()).as_secs()),
            total_runs: state.total_runs,
            success_runs: state.success_runs,
            failed_runs: state.failed_runs,
        }
    }

    async fn perform_sync(
        config: &Arc<RwLock<TomlConfig>>,
        sync_service: &Arc<RwLock<BillSyncService>>,
        state: &Arc<RwLock<AutoSyncState>>,
    ) {
        let (identity_id, captcha_mode, sync_range) = {
            let cfg = config.read().await;
            let app_cfg = cfg.get();
            (
                app_cfg.identity.default_identity_id,
                app_cfg.captcha.mode.clone(),
                match app_cfg.sync.auto_sync_range {
                    SyncRangePresetConfig::Week => SyncRangePreset::Week,
                    SyncRangePresetConfig::HalfMonth => SyncRangePreset::HalfMonth,
                    SyncRangePresetConfig::Month => SyncRangePreset::Month,
                    SyncRangePresetConfig::HalfYear => SyncRangePreset::HalfYear,
                    SyncRangePresetConfig::Year => SyncRangePreset::Year,
                    SyncRangePresetConfig::All => SyncRangePreset::All,
                },
            )
        };

        if identity_id == 0 {
            tracing::info!("[AutoSync] 未设置默认身份，跳过自动同步");
            return;
        }

        if matches!(captcha_mode, CaptchaMode::Manual) {
            tracing::info!("[AutoSync] 当前为手动验证码模式，跳过自动同步");
            return;
        }

        tracing::info!(
            "[AutoSync] 开始自动同步 | identity_id={} | range={:?}",
            identity_id,
            sync_range
        );

        let result = {
            let service = sync_service.read().await;
            service.sync_identity(identity_id, sync_range, None).await
        };

        let mut service_state = state.write().await;
        service_state.last_run = Some(Instant::now());
        service_state.next_run = None;
        service_state.total_runs += 1;

        match result {
            Ok(result) => {
                service_state.success_runs += 1;
                tracing::info!(
                    "[AutoSync] 自动同步完成 | identity_id={} | new_items={}",
                    identity_id,
                    result.total_new_count
                );
            }
            Err(error) => {
                service_state.failed_runs += 1;
                tracing::error!(
                    "[AutoSync] 自动同步失败 | identity_id={} | error={}",
                    identity_id,
                    error
                );
            }
        }
    }
}

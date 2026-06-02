use serde::{Deserialize, Serialize};
// use shmtu_cas::datatype::bill::BillType;
// use shmtu_cas::sync::SyncOptions;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;
use crate::sync::{SyncProgress, SyncProgressCallback, SyncRangePreset, SyncStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgressFrontend {
    pub account_id: String,
    pub current_account: String,
    pub account_index: u32,
    pub total_accounts: u32,
    pub current_page: u32,
    pub total_pages: u32,
    pub new_items: usize,
    pub is_running: bool,
    pub status: String,
    pub message: Option<String>,
    pub error: Option<String>,
    pub captcha_required: bool,
    pub captcha_image: Option<String>,
    pub execution: Option<String>,
}

impl SyncProgressFrontend {
    fn idle() -> Self {
        Self {
            account_id: String::new(),
            current_account: String::new(),
            account_index: 0,
            total_accounts: 0,
            current_page: 0,
            total_pages: 0,
            new_items: 0,
            is_running: false,
            status: "idle".to_string(),
            message: None,
            error: None,
            captcha_required: false,
            captcha_image: None,
            execution: None,
        }
    }

    fn captcha_required(image: String, execution: String, message: &str) -> Self {
        Self {
            account_id: String::new(),
            current_account: String::new(),
            account_index: 0,
            total_accounts: 0,
            current_page: 0,
            total_pages: 0,
            new_items: 0,
            is_running: false,
            status: "captcha_required".to_string(),
            message: Some(message.to_string()),
            error: Some(message.to_string()),
            captcha_required: true,
            captcha_image: Some(image),
            execution: Some(execution),
        }
    }

    fn success(new_items: usize) -> Self {
        Self {
            account_id: String::new(),
            current_account: String::new(),
            account_index: 0,
            total_accounts: 0,
            current_page: 0,
            total_pages: 0,
            new_items,
            is_running: false,
            status: "completed".to_string(),
            message: Some(format!("同步完成，本次新增 {} 条记录", new_items)),
            error: None,
            captcha_required: false,
            captcha_image: None,
            execution: None,
        }
    }
}

impl SyncProgressFrontend {
    fn from_runtime(progress: SyncProgress) -> Self {
        let (status, current_page, total_pages, is_running, new_items, error, message) =
            match progress.status {
                SyncStatus::ProbingLogin => (
                    "running".to_string(),
                    0,
                    0,
                    true,
                    progress.total_new_count,
                    None,
                    Some(format!(
                        "正在检查账号 {} 的登录状态（{}/{}）...",
                        progress.current_account,
                        progress.account_index + 1,
                        progress.total_accounts
                    )),
                ),
                SyncStatus::GettingCaptcha => (
                    "captcha_required".to_string(),
                    0,
                    0,
                    false,
                    progress.total_new_count,
                    Some("请输入验证码".to_string()),
                    Some(format!(
                        "账号 {} 需要验证码（{}/{}）",
                        progress.current_account,
                        progress.account_index + 1,
                        progress.total_accounts
                    )),
                ),
                SyncStatus::LoggingIn => (
                    "running".to_string(),
                    0,
                    0,
                    true,
                    progress.total_new_count,
                    None,
                    Some(format!(
                        "账号 {} 已通过登录检查，正在准备拉取账单（{}/{}）...",
                        progress.current_account,
                        progress.account_index + 1,
                        progress.total_accounts
                    )),
                ),
                SyncStatus::Syncing { page, total } => (
                    "running".to_string(),
                    page,
                    total,
                    true,
                    progress.new_count,
                    None,
                    Some(format!(
                        "账号 {} 正在从校园平台拉取账单第 {}/{} 页，当前账号新增 {} 条，累计新增 {} 条（{}/{}）...",
                        progress.current_account,
                        page,
                        total,
                        progress.new_count,
                        progress.total_new_count,
                        progress.account_index + 1,
                        progress.total_accounts
                    )),
                ),
                SyncStatus::Persisting => (
                    "running".to_string(),
                    progress.pages_fetched,
                    progress.pages_fetched,
                    true,
                    progress.new_count,
                    None,
                    Some(format!(
                        "账号 {} 已拉取完成，正在写入原始账单并合并到身份：新增 {} 条，拉取 {} 页，累计新增 {} 条（{}/{}）",
                        progress.current_account,
                        progress.new_count,
                        progress.pages_fetched,
                        progress.total_new_count,
                        progress.account_index + 1,
                        progress.total_accounts
                    )),
                ),
                SyncStatus::Completed => (
                    "running".to_string(),
                    progress.pages_fetched,
                    progress.pages_fetched,
                    true,
                    progress.total_new_count,
                    None,
                    Some(format!(
                        "账号 {} 拉取完成并已写入原始账单、合并到身份：新增 {} 条，拉取 {} 页，累计新增 {} 条（{}/{}）",
                        progress.current_account,
                        progress.new_count,
                        progress.pages_fetched,
                        progress.total_new_count,
                        progress.account_index + 1,
                        progress.total_accounts
                    )),
                ),
                SyncStatus::Failed(err) => (
                    "error".to_string(),
                    0,
                    0,
                    false,
                    progress.total_new_count,
                    Some(err.clone()),
                    Some(err),
                ),
            };

        Self {
            account_id: progress.account_id,
            current_account: progress.current_account,
            account_index: progress.account_index as u32,
            total_accounts: progress.total_accounts as u32,
            current_page,
            total_pages,
            new_items,
            is_running,
            status,
            message,
            error,
            captcha_required: false,
            captcha_image: None,
            execution: None,
        }
    }
}

fn create_progress_callback(app: AppHandle) -> SyncProgressCallback {
    Box::new(move |progress: SyncProgress| {
        let _ = app.emit(
            "sync-progress",
            SyncProgressFrontend::from_runtime(progress),
        );
    })
}

fn parse_captcha_marker<'a>(error: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    let start = error.find(marker)?;
    let payload = &error[start + marker.len()..];
    let mut parts = payload.splitn(2, '|');
    let image = parts.next()?;
    let execution = parts.next()?;
    Some((image, execution))
}

#[tauri::command]
pub async fn incremental_sync(
    state: State<'_, AppState>,
    app: AppHandle,
    identity_id: i64,
    sync_range: SyncRangePreset,
) -> Result<SyncProgressFrontend, String> {
    tracing::info!(
        "[Command] incremental_sync called, identity_id={}, sync_range={:?}",
        identity_id,
        sync_range
    );

    let sync_service = state.sync_service.read().await;
    let progress_callback = create_progress_callback(app.clone());
    let result = sync_service
        .sync_identity(identity_id, sync_range, Some(&progress_callback))
        .await;

    match result {
        Ok(r) => {
            tracing::info!(
                "[Command] incremental_sync service returned {} account results",
                r.results.len()
            );
            let _ = app.emit(
                "sync-progress",
                SyncProgressFrontend::success(r.total_new_count),
            );
            tracing::info!(
                "[Command] incremental_sync completed, new_items={}",
                r.total_new_count
            );
            Ok(SyncProgressFrontend::success(r.total_new_count))
        }
        Err(e) => {
            let err_str = e.to_string();
            if let Some((image, execution)) =
                parse_captcha_marker(&err_str, "MANUAL_CAPTCHA_REQUIRED|")
            {
                tracing::info!(
                    "[Command] sync_identity requires manual captcha, image_len={}, execution_len={}",
                    image.len(),
                    execution.len()
                );
                return Ok(SyncProgressFrontend::captcha_required(
                    image.to_string(),
                    execution.to_string(),
                    "请输入验证码",
                ));
            }
            tracing::error!("[Command] sync_identity FAILED: [{}]", err_str);
            Err(err_str)
        }
    }
}

#[tauri::command]
pub async fn full_sync(
    state: State<'_, AppState>,
    app: AppHandle,
    identity_id: i64,
    sync_range: SyncRangePreset,
) -> Result<SyncProgressFrontend, String> {
    tracing::info!(
        "[Command] full_sync called, identity_id={}, sync_range={:?}",
        identity_id,
        sync_range
    );

    let sync_service = state.sync_service.read().await;
    let progress_callback = create_progress_callback(app.clone());
    let result = sync_service
        .full_sync_identity(identity_id, sync_range, Some(&progress_callback))
        .await;

    match result {
        Ok(r) => {
            tracing::info!(
                "[Command] full_sync service returned {} account results",
                r.results.len()
            );
            let _ = app.emit(
                "sync-progress",
                SyncProgressFrontend::success(r.total_new_count),
            );
            tracing::info!(
                "[Command] full_sync completed, new_items={}",
                r.total_new_count
            );
            Ok(SyncProgressFrontend::success(r.total_new_count))
        }
        Err(e) => {
            let err_str = e.to_string();
            if let Some((image, execution)) =
                parse_captcha_marker(&err_str, "MANUAL_CAPTCHA_REQUIRED|")
            {
                tracing::info!(
                    "[Command] full_sync requires manual captcha, image_len={}, execution_len={}",
                    image.len(),
                    execution.len()
                );
                return Ok(SyncProgressFrontend::captcha_required(
                    image.to_string(),
                    execution.to_string(),
                    "请输入验证码",
                ));
            }
            tracing::error!("[Command] full_sync FAILED: [{}]", err_str);
            Err(err_str)
        }
    }
}

/// 增量同步单个账号
#[tauri::command]
pub async fn incremental_sync_account(
    state: State<'_, AppState>,
    app: AppHandle,
    identity_id: i64,
    account_id: String,
    sync_range: SyncRangePreset,
) -> Result<SyncProgressFrontend, String> {
    tracing::info!(
        "[Command] incremental_sync_account called, identity_id={}, account_id={}, sync_range={:?}",
        identity_id,
        account_id,
        sync_range
    );

    let sync_service = state.sync_service.read().await;
    let progress_callback = create_progress_callback(app.clone());
    let result = sync_service
        .sync_single_account_by_id(
            identity_id,
            &account_id,
            sync_range,
            Some(&progress_callback),
        )
        .await;

    match result {
        Ok(r) => {
            tracing::info!(
                "[Command] incremental_sync_account service returned {} account results",
                r.results.len()
            );
            let _ = app.emit(
                "sync-progress",
                SyncProgressFrontend::success(r.total_new_count),
            );
            tracing::info!(
                "[Command] incremental_sync_account completed, account_id={}, new_items={}",
                account_id,
                r.total_new_count
            );
            Ok(SyncProgressFrontend::success(r.total_new_count))
        }
        Err(e) => {
            let err_str = e.to_string();
            if let Some((image, execution)) =
                parse_captcha_marker(&err_str, "MANUAL_CAPTCHA_REQUIRED|")
            {
                tracing::info!(
                    "[Command] incremental_sync_account requires manual captcha, image_len={}, execution_len={}",
                    image.len(),
                    execution.len()
                );
                return Ok(SyncProgressFrontend::captcha_required(
                    image.to_string(),
                    execution.to_string(),
                    "请输入验证码",
                ));
            }
            tracing::error!("[Command] incremental_sync_account FAILED: [{}]", err_str);
            Err(err_str)
        }
    }
}

/// 全量同步单个账号（清除旧数据后重新同步）
#[tauri::command]
pub async fn full_sync_account(
    state: State<'_, AppState>,
    app: AppHandle,
    identity_id: i64,
    account_id: String,
    sync_range: SyncRangePreset,
) -> Result<SyncProgressFrontend, String> {
    tracing::info!(
        "[Command] full_sync_account called, identity_id={}, account_id={}, sync_range={:?}",
        identity_id,
        account_id,
        sync_range
    );

    let sync_service = state.sync_service.read().await;
    let progress_callback = create_progress_callback(app.clone());
    let result = sync_service
        .full_sync_single_account(
            identity_id,
            &account_id,
            sync_range,
            Some(&progress_callback),
        )
        .await;

    match result {
        Ok(r) => {
            tracing::info!(
                "[Command] full_sync_account service returned {} account results",
                r.results.len()
            );
            let _ = app.emit(
                "sync-progress",
                SyncProgressFrontend::success(r.total_new_count),
            );
            tracing::info!(
                "[Command] full_sync_account completed, account_id={}, new_items={}",
                account_id,
                r.total_new_count
            );
            Ok(SyncProgressFrontend::success(r.total_new_count))
        }
        Err(e) => {
            let err_str = e.to_string();
            if let Some((image, execution)) =
                parse_captcha_marker(&err_str, "MANUAL_CAPTCHA_REQUIRED|")
            {
                tracing::info!(
                    "[Command] full_sync_account requires manual captcha, image_len={}, execution_len={}",
                    image.len(),
                    execution.len()
                );
                return Ok(SyncProgressFrontend::captcha_required(
                    image.to_string(),
                    execution.to_string(),
                    "请输入验证码",
                ));
            }
            tracing::error!("[Command] full_sync_account FAILED: [{}]", err_str);
            Err(err_str)
        }
    }
}

#[tauri::command]
pub async fn get_sync_progress() -> Result<SyncProgressFrontend, String> {
    Ok(SyncProgressFrontend::idle())
}

/// 使用手动输入的验证码完成登录并同步
#[tauri::command]
pub async fn sync_with_captcha(
    state: State<'_, AppState>,
    app: AppHandle,
    identity_id: i64,
    captcha_code: String,
    execution: String,
) -> Result<SyncProgressFrontend, String> {
    tracing::info!(
        "[Command] sync_with_captcha called, identity_id={}, captcha_len={}",
        identity_id,
        captcha_code.len()
    );

    let sync_service = state.sync_service.read().await;
    let progress_callback = create_progress_callback(app.clone());

    match sync_service
        .sync_with_captcha(
            identity_id,
            &captcha_code,
            &execution,
            Some(&progress_callback),
        )
        .await
    {
        Ok(result) => {
            tracing::info!(
                "[Command] sync_with_captcha service returned {} account results",
                result.results.len()
            );
            let _ = app.emit(
                "sync-progress",
                SyncProgressFrontend::success(result.total_new_count),
            );
            tracing::info!(
                "[Command] sync_with_captcha SUCCESS, new_items={}",
                result.total_new_count
            );
            Ok(SyncProgressFrontend::success(result.total_new_count))
        }
        Err(e) => {
            let err_str = e.to_string();
            if let Some((image, execution)) = parse_captcha_marker(&err_str, "CAPTCHA_WRONG|") {
                tracing::warn!(
                    "[Command] sync_with_captcha captcha wrong, refreshing image image_len={}, execution_len={}",
                    image.len(),
                    execution.len()
                );
                return Ok(SyncProgressFrontend::captcha_required(
                    image.to_string(),
                    execution.to_string(),
                    "验证码错误，请重新输入",
                ));
            }
            if let Some((image, execution)) =
                parse_captcha_marker(&err_str, "MANUAL_CAPTCHA_REQUIRED|")
            {
                tracing::info!(
                    "[Command] sync_with_captcha requires captcha for next account, image_len={}, execution_len={}",
                    image.len(),
                    execution.len()
                );
                return Ok(SyncProgressFrontend::captcha_required(
                    image.to_string(),
                    execution.to_string(),
                    "请输入验证码",
                ));
            }
            tracing::error!("[Command] sync_with_captcha FAILED: [{}]", err_str);
            Err(err_str)
        }
    }
}

/// 刷新验证码（用于手动模式）
#[tauri::command]
pub async fn refresh_captcha(state: State<'_, AppState>) -> Result<SyncProgressFrontend, String> {
    tracing::info!("[Command] refresh_captcha called");

    let sync_service = state.sync_service.read().await;

    match sync_service.get_captcha_for_manual_login().await {
        Ok((image, execution)) => {
            tracing::info!(
                "[Command] refresh_captcha success image_len={}, execution_len={}",
                image.len(),
                execution.len()
            );
            Ok(SyncProgressFrontend::captcha_required(
                image,
                execution,
                "请输入验证码",
            ))
        }
        Err(e) => {
            tracing::error!("[Command] refresh_captcha FAILED: {}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn cas_login(
    state: State<'_, AppState>,
    account_id: String,
    _password: String,
    captcha_code: String,
) -> Result<bool, String> {
    tracing::info!("[Command] cas_login called, account_id={}", account_id);

    let db = state.db_manager.read().await;
    let account = db
        .get_account_by_student_id(&account_id)
        .await
        .map_err(|e| {
            tracing::error!("[Command] cas_login: account not found: {}", e);
            e.to_string()
        })?
        .ok_or_else(|| {
            tracing::error!("[Command] cas_login: account {} does not exist", account_id);
            "账号不存在".to_string()
        })?;

    let sync_service = state.sync_service.read().await;
    sync_service
        .login_with_captcha(&account, &captcha_code)
        .await
        .map_err(|e| {
            tracing::error!("[Command] cas_login FAILED for {}: {}", account_id, e);
            e.to_string()
        })?;

    tracing::info!("[Command] cas_login SUCCESS for {}", account_id);
    Ok(true)
}

#[tauri::command]
pub async fn check_login_status(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<bool, String> {
    tracing::debug!(
        "[Command] check_login_status called, account_id={}",
        account_id
    );

    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;
    let session = db.get_session(&account_id, &crypto).await.map_err(|e| {
        tracing::error!("[Command] check_login_status: get_session failed: {}", e);
        e.to_string()
    })?;

    let result = session.as_ref().map(|s| s.is_valid).unwrap_or(false);
    tracing::debug!("[Command] check_login_status result: {}", result);
    Ok(result)
}

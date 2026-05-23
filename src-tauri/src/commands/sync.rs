use serde::{Deserialize, Serialize};
// use shmtu_cas::datatype::bill::BillType;
// use shmtu_cas::sync::SyncOptions;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgressFrontend {
    pub account_id: String,
    pub current_page: u32,
    pub total_pages: u32,
    pub new_items: usize,
    pub is_running: bool,
    pub status: String,
    pub error: Option<String>,
    pub captcha_required: bool,
    pub captcha_image: Option<String>,
    pub execution: Option<String>,
}

impl SyncProgressFrontend {
    fn idle() -> Self {
        Self {
            account_id: String::new(),
            current_page: 0,
            total_pages: 0,
            new_items: 0,
            is_running: false,
            status: "idle".to_string(),
            error: None,
            captcha_required: false,
            captcha_image: None,
            execution: None,
        }
    }

    fn captcha_required(image: String, execution: String, message: &str) -> Self {
        Self {
            account_id: String::new(),
            current_page: 0,
            total_pages: 0,
            new_items: 0,
            is_running: false,
            status: "captcha_required".to_string(),
            error: Some(message.to_string()),
            captcha_required: true,
            captcha_image: Some(image),
            execution: Some(execution),
        }
    }

    fn success(new_items: usize) -> Self {
        Self {
            account_id: String::new(),
            current_page: 0,
            total_pages: 0,
            new_items,
            is_running: false,
            status: "completed".to_string(),
            error: None,
            captcha_required: false,
            captcha_image: None,
            execution: None,
        }
    }
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
) -> Result<SyncProgressFrontend, String> {
    tracing::info!(
        "[Command] incremental_sync called, identity_id={}",
        identity_id
    );

    let sync_service = state.sync_service.read().await;
    let result = sync_service.sync_identity(identity_id, None).await;

    match result {
        Ok(r) => {
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
                tracing::info!("[Command] sync_identity requires manual captcha");
                let progress = SyncProgressFrontend::captcha_required(
                    image.to_string(),
                    execution.to_string(),
                    "请输入验证码",
                );
                let _ = app.emit("sync-progress", progress.clone());
                return Ok(progress);
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
) -> Result<SyncProgressFrontend, String> {
    tracing::info!("[Command] full_sync called, identity_id={}", identity_id);

    let sync_service = state.sync_service.read().await;
    let result = sync_service
        .full_sync_identity(identity_id, None)
        .await
        .map_err(|e| {
            let err_str = e.to_string();
            tracing::error!("[Command] full_sync FAILED: [{}]", err_str);
            err_str
        })?;

    let _ = app.emit(
        "sync-progress",
        SyncProgressFrontend::success(result.total_new_count),
    );
    tracing::info!(
        "[Command] full_sync completed, new_items={}",
        result.total_new_count
    );
    Ok(SyncProgressFrontend::success(result.total_new_count))
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

    match sync_service
        .sync_with_captcha(identity_id, &captcha_code, &execution)
        .await
    {
        Ok(result) => {
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
                tracing::warn!("[Command] sync_with_captcha captcha wrong, refreshing image");
                return Ok(SyncProgressFrontend::captcha_required(
                    image.to_string(),
                    execution.to_string(),
                    "验证码错误，请重新输入",
                ));
            }
            if let Some((image, execution)) =
                parse_captcha_marker(&err_str, "MANUAL_CAPTCHA_REQUIRED|")
            {
                tracing::info!("[Command] sync_with_captcha requires captcha for next account");
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
            tracing::info!("[Command] refresh_captcha success");
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
    let session = db.get_session(&account_id, &crypto).map_err(|e| {
        tracing::error!("[Command] check_login_status: get_session failed: {}", e);
        e.to_string()
    })?;

    let result = session.as_ref().map(|s| s.is_valid).unwrap_or(false);
    tracing::debug!("[Command] check_login_status result: {}", result);
    Ok(result)
}

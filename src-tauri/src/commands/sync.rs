use serde::{Deserialize, Serialize};
use shmtu_cas::datatype::bill::BillType;
use shmtu_cas::sync::SyncOptions;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

/// 前端同步进度（与 tauri.ts SyncProgress 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgressFrontend {
    pub account_id: String,
    pub current_page: u32,
    pub total_pages: u32,
    pub new_items: usize,
    pub is_running: bool,
    pub status: String,
    pub error: Option<String>,
    /// 是否需要手动输入验证码
    pub captcha_required: bool,
    /// 验证码图片（base64，不含前缀）
    pub captcha_image: Option<String>,
    /// CAS execution token
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
}

#[tauri::command]
pub async fn incremental_sync(
    state: State<'_, AppState>,
    app: AppHandle,
    identity_id: i64,
) -> Result<SyncProgressFrontend, String> {
    let config = state.config.read().await;
    let sync_config = &config.get().sync;
    let sync_options = SyncOptions {
        start_page: 1,
        max_pages: sync_config.max_pages,
        bill_type: BillType::All,
        early_stop_threshold: sync_config.early_stop_threshold,
    };
    drop(config);

    let sync_service = state.sync_service.read().await;
    
    // 尝试同步，如果需要验证码则返回验证码信息
    match sync_service.sync_identity(identity_id, &sync_options, None).await {
        Ok(result) => {
            let _ = app.emit(
                "sync-progress",
                SyncProgressFrontend {
                    account_id: String::new(),
                    current_page: 0,
                    total_pages: 0,
                    new_items: result.total_new_count,
                    is_running: false,
                    status: "completed".to_string(),
                    error: None,
                    captcha_required: false,
                    captcha_image: None,
                    execution: None,
                },
            );

            Ok(SyncProgressFrontend {
                account_id: String::new(),
                current_page: 0,
                total_pages: 0,
                new_items: result.total_new_count,
                is_running: false,
                status: "completed".to_string(),
                error: None,
                captcha_required: false,
                captcha_image: None,
                execution: None,
            })
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("手动模式需要前端交互") {
                tracing::info!("检测到手动模式，开始获取验证码...");
                // 获取验证码图片和 execution
                match sync_service.get_captcha_for_manual_login().await {
                    Ok((image, execution)) => {
                        tracing::info!("手动模式：返回验证码给前端");
                        Ok(SyncProgressFrontend {
                            account_id: String::new(),
                            current_page: 0,
                            total_pages: 0,
                            new_items: 0,
                            is_running: false,
                            status: "captcha_required".to_string(),
                            error: Some("需要手动输入验证码".to_string()),
                            captcha_required: true,
                            captcha_image: Some(image),
                            execution: Some(execution),
                        })
                    }
                    Err(captcha_err) => {
                        tracing::error!("获取验证码失败: {}", captcha_err);
                        Err(format!("获取验证码失败: {}", captcha_err))
                    }
                }
            } else {
                tracing::error!("同步失败，返回错误: [{}]", err_str);
                Err(err_str)
            }
        }
    }
}

#[tauri::command]
pub async fn full_sync(
    state: State<'_, AppState>,
    app: AppHandle,
    identity_id: i64,
) -> Result<SyncProgressFrontend, String> {
    let sync_service = state.sync_service.read().await;
    
    match sync_service.full_sync_identity(identity_id, None).await {
        Ok(result) => {
            let _ = app.emit(
                "sync-progress",
                SyncProgressFrontend {
                    account_id: String::new(),
                    current_page: 0,
                    total_pages: 0,
                    new_items: result.total_new_count,
                    is_running: false,
                    status: "completed".to_string(),
                    error: None,
                    captcha_required: false,
                    captcha_image: None,
                    execution: None,
                },
            );

            Ok(SyncProgressFrontend {
                account_id: String::new(),
                current_page: 0,
                total_pages: 0,
                new_items: result.total_new_count,
                is_running: false,
                status: "completed".to_string(),
                error: None,
                captcha_required: false,
                captcha_image: None,
                execution: None,
            })
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("手动模式需要前端交互") {
                match sync_service.get_captcha_for_manual_login().await {
                    Ok((image, execution)) => {
                        tracing::info!("手动模式：返回验证码给前端");
                        Ok(SyncProgressFrontend {
                            account_id: String::new(),
                            current_page: 0,
                            total_pages: 0,
                            new_items: 0,
                            is_running: false,
                            status: "captcha_required".to_string(),
                            error: Some("需要手动输入验证码".to_string()),
                            captcha_required: true,
                            captcha_image: Some(image),
                            execution: Some(execution),
                        })
                    }
                    Err(captcha_err) => {
                        tracing::error!("获取验证码失败: {}", captcha_err);
                        Err(format!("获取验证码失败: {}", captcha_err))
                    }
                }
            } else {
                Err(err_str)
            }
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
    let sync_service = state.sync_service.read().await;
    
    match sync_service.sync_with_captcha(identity_id, &captcha_code, &execution).await {
        Ok(result) => {
            let _ = app.emit(
                "sync-progress",
                SyncProgressFrontend {
                    account_id: String::new(),
                    current_page: 0,
                    total_pages: 0,
                    new_items: result.total_new_count,
                    is_running: false,
                    status: "completed".to_string(),
                    error: None,
                    captcha_required: false,
                    captcha_image: None,
                    execution: None,
                },
            );

            Ok(SyncProgressFrontend {
                account_id: String::new(),
                current_page: 0,
                total_pages: 0,
                new_items: result.total_new_count,
                is_running: false,
                status: "completed".to_string(),
                error: None,
                captcha_required: false,
                captcha_image: None,
                execution: None,
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn cas_login(
    state: State<'_, AppState>,
    account_id: String,
    _password: String,
    captcha_code: String,
) -> Result<bool, String> {
    let db = state.db_manager.read().await;
    let account = db
        .get_account_by_student_id(&account_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    let sync_service = state.sync_service.read().await;
    sync_service
        .login_with_captcha(&account, &captcha_code)
        .await
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub async fn check_login_status(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<bool, String> {
    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;
    let session = db
        .get_session(&account_id, &crypto)
        .map_err(|e| e.to_string())?;

    Ok(session.as_ref().map(|s| s.is_valid).unwrap_or(false))
}

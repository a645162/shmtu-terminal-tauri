use tauri::State;

use crate::models::{CreateIdentityParams, Identity};
use crate::state::AppState;

#[tauri::command]
pub async fn list_identities(state: State<'_, AppState>) -> Result<Vec<Identity>, String> {
    let db = state.db_manager.read().await;
    db.list_identities().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_identity(state: State<'_, AppState>, name: String) -> Result<Identity, String> {
    let db = state.db_manager.read().await;
    let params = CreateIdentityParams {
        name,
        enable: Some(true),
        enable_update: Some(true),
        birthday: None,
        default_remember: Some(false),
    };
    let id = db.create_identity(&params).map_err(|e| e.to_string())?;
    let identity = db.get_identity(id).map_err(|e| e.to_string())?;
    identity.ok_or_else(|| "创建身份后未找到".to_string())
}

#[tauri::command]
pub async fn update_identity(state: State<'_, AppState>, identity: Identity) -> Result<(), String> {
    let db = state.db_manager.read().await;
    db.update_identity(&identity).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_identity(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = state.db_manager.read().await;
    db.delete_identity(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_default_identity(
    state: State<'_, AppState>,
    identity_id: i64,
) -> Result<(), String> {
    let mut config = state.config.write().await;
    config
        .set_default_identity(identity_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_default_identity(state: State<'_, AppState>) -> Result<Option<Identity>, String> {
    let config = state.config.read().await;
    let app_config = config.get();
    if !app_config.identity.remember_default {
        return Ok(None);
    }
    let identity_id = app_config.identity.default_identity_id;
    if identity_id == 0 {
        return Ok(None);
    }
    let db = state.db_manager.read().await;
    db.get_identity(identity_id).map_err(|e| e.to_string())
}

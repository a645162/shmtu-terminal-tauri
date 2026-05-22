use tauri::State;

use crate::config::AppConfig;
use crate::state::AppState;

#[tauri::command]
pub async fn load_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.read().await;
    Ok(config.get().clone())
}

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    let mut cfg = state.config.write().await;
    cfg.update(config).map_err(|e| e.to_string())?;

    // 重新加载分类器
    drop(cfg);
    state.reload_classifier().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn verify_startup_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<bool, String> {
    let config = state.config.read().await;
    Ok(config.verify_startup_password(&password))
}

#[tauri::command]
pub async fn set_startup_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<(), String> {
    let mut config = state.config.write().await;
    config
        .set_startup_password(&password)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_app_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
pub async fn check_for_updates() -> Result<Option<String>, String> {
    // TODO: 实现 GitHub Releases 版本检查
    Ok(None)
}

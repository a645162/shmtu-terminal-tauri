use tauri::State;

use crate::cloud::backup::RestoreReport;
use crate::cloud::webdav::{BackupMeta, WebDavConfig};
use crate::state::AppState;

#[tauri::command]
pub async fn cloud_backup_get_config(state: State<'_, AppState>) -> Result<WebDavConfig, String> {
    let config = state.config.read().await;
    Ok(config.get().cloud_backup.webdav.clone())
}

#[tauri::command]
pub async fn cloud_backup_save_config(
    state: State<'_, AppState>,
    config: WebDavConfig,
) -> Result<(), String> {
    state.cloud_backup.write().await.configure_webdav(config.clone());
    let mut cfg = state.config.write().await;
    cfg.get_mut().cloud_backup.webdav = config;
    cfg.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_backup_test_connection(state: State<'_, AppState>) -> Result<bool, String> {
    let manager = state.cloud_backup.read().await;
    manager.test_connection().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_backup_test_write_read(state: State<'_, AppState>) -> Result<String, String> {
    let manager = state.cloud_backup.read().await;
    manager.test_write_read().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_backup_now(
    state: State<'_, AppState>,
    password: Option<String>,
) -> Result<String, String> {
    let manager = state.cloud_backup.read().await;
    let config = state.config.read().await;
    let data_dir = config.data_directory();
    let backup_root = config.get().cloud_backup.webdav.backup_root.clone();
    let result = manager.backup_now(&data_dir, 0, password.as_deref(), &backup_root)
        .await.map_err(|e| e.to_string())?;
    let max_keep = config.get().cloud_backup.max_keep;
    let _ = manager.prune_old_backups(&backup_root, max_keep).await;
    Ok(format!("✓ 备份完成 ({} 字节)", result.bytes))
}

#[tauri::command]
pub async fn cloud_backup_restore(
    state: State<'_, AppState>,
    remote_path: String,
    password: Option<String>,
) -> Result<RestoreReport, String> {
    let manager = state.cloud_backup.read().await;
    let config = state.config.read().await;
    let data_dir = config.data_directory();
    manager.restore_backup(&remote_path, password.as_deref(), &data_dir)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_backup_list_remote(state: State<'_, AppState>) -> Result<Vec<BackupMeta>, String> {
    let manager = state.cloud_backup.read().await;
    let config = state.config.read().await;
    let backup_root = config.get().cloud_backup.webdav.backup_root.clone();
    manager.list_remote_backups(&backup_root).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_backup_delete_remote(
    state: State<'_, AppState>,
    remote_path: String,
) -> Result<bool, String> {
    let manager = state.cloud_backup.read().await;
    manager.delete_remote_backup(&remote_path).await.map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CloudBackupAutoConfig {
    pub auto_enabled: bool,
    pub auto_interval_minutes: u64,
    pub max_keep: usize,
}

#[tauri::command]
pub async fn cloud_backup_get_auto_config(state: State<'_, AppState>) -> Result<CloudBackupAutoConfig, String> {
    let config = state.config.read().await;
    let cb = &config.get().cloud_backup;
    Ok(CloudBackupAutoConfig {
        auto_enabled: cb.auto_enabled,
        auto_interval_minutes: cb.auto_interval_minutes,
        max_keep: cb.max_keep,
    })
}

#[tauri::command]
pub async fn cloud_backup_set_auto_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut cfg = state.config.write().await;
    cfg.get_mut().cloud_backup.auto_enabled = enabled;
    cfg.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_backup_set_auto_interval(state: State<'_, AppState>, minutes: u64) -> Result<(), String> {
    let mut cfg = state.config.write().await;
    cfg.get_mut().cloud_backup.auto_interval_minutes = minutes.max(15);
    cfg.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_backup_set_max_keep(state: State<'_, AppState>, count: usize) -> Result<(), String> {
    let mut cfg = state.config.write().await;
    cfg.get_mut().cloud_backup.max_keep = count.clamp(1, 100);
    cfg.save().map_err(|e| e.to_string())
}

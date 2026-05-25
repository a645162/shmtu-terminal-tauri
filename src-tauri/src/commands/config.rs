use tauri::State;

use crate::config::AppConfig;
use crate::session_refresh::{SessionExpirationResult, SessionExpirationStatus};
use crate::state::AppState;

#[tauri::command]
pub async fn get_session_expiration_status(
    state: State<'_, AppState>,
) -> Result<SessionExpirationStatus, String> {
    Ok(state.session_expiration_service.get_status().await)
}

#[tauri::command]
pub async fn check_session_expiration(
    state: State<'_, AppState>,
) -> Result<SessionExpirationResult, String> {
    Ok(state.session_expiration_service.check_now().await)
}

#[tauri::command]
pub async fn restart_session_expiration_service(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.session_expiration_service.restart().await;
    Ok(())
}

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
    state.reload_classifier().await.map_err(|e| e.to_string())?;

    // 重启 session 过期检查服务（配置可能已变更）
    state.session_expiration_service.restart().await;
    Ok(())
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

/// 检查更新信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateInfo {
    /// 最新版本号
    pub version: String,
    /// 发布说明
    pub release_notes: String,
    /// 下载地址
    pub download_url: String,
    /// 是否是新版本
    pub is_newer: bool,
}

#[tauri::command]
pub async fn check_for_updates() -> Result<Option<UpdateInfo>, String> {
    // 从 GitHub Releases API 获取最新版本
    let github_api = "https://api.github.com/repos/konghaomin/shmtu-terminal-tauri/releases/latest";

    let client = reqwest::Client::builder()
        .user_agent("shmtu-terminal/1.0")
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    let response = client
        .get(github_api)
        .send()
        .await
        .map_err(|e| format!("检查更新失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("GitHub API 返回错误: {}", response.status()));
    }

    #[derive(serde::Deserialize)]
    struct GitHubRelease {
        tag_name: String,
        body: Option<String>,
        assets: Vec<GitHubAsset>,
    }

    #[derive(serde::Deserialize)]
    struct GitHubAsset {
        browser_download_url: String,
        name: String,
    }

    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    // 解析版本号（去掉 v 前缀）
    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let current_version = env!("CARGO_PKG_VERSION");

    // 比较版本号
    let is_newer = compare_versions(&latest_version, current_version) > 0;

    // 获取下载链接
    let download_url = release
        .assets
        .iter()
        .find(|a| {
            a.name.contains("linux") && (a.name.contains(".deb") || a.name.contains(".AppImage"))
        })
        .map(|a| a.browser_download_url.clone())
        .unwrap_or_default();

    // 获取发布说明（限制长度）
    let release_notes = release
        .body
        .unwrap_or_default()
        .chars()
        .take(500)
        .collect::<String>();

    Ok(Some(UpdateInfo {
        version: latest_version,
        release_notes,
        download_url,
        is_newer,
    }))
}

/// 比较两个语义化版本号
/// 返回正数表示 v1 > v2，负数表示 v1 < v2，0 表示相等
fn compare_versions(v1: &str, v2: &str) -> i32 {
    let parts1: Vec<u32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
    let parts2: Vec<u32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();

    let max_len = parts1.len().max(parts2.len());
    for i in 0..max_len {
        let p1 = parts1.get(i).unwrap_or(&0);
        let p2 = parts2.get(i).unwrap_or(&0);
        if p1 != p2 {
            return (*p1 as i32) - (*p2 as i32);
        }
    }
    0
}

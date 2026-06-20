//! OCR HTTP 服务器管理 Tauri 命令
//!
//! 暴露给前端 SettingsDialog 用,允许启用/停用 / 修改端口 / 查询状态 / 轮换 token。

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::state::AppState;

pub type OcrHttpServerHandle = Arc<crate::ocr_server::OcrHttpServerManager>;

#[derive(Debug, Clone, Serialize)]
pub struct OcrServerStatus {
    pub running: bool,
    pub enabled: bool,
    pub port: u16,
    pub token: String,
    pub models_loaded: bool,
    pub model_version: Option<String>,
    pub total_requests: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_response_ms: f64,
    pub url: String,
    pub ips: Vec<String>,
}

#[tauri::command]
pub async fn ocr_server_start(
    port: Option<u16>,
    app_state: State<'_, AppState>,
    manager: State<'_, OcrHttpServerHandle>,
) -> Result<OcrServerStatus, String> {
    let desired_port = port.unwrap_or(app_state.config.blocking_read().get().captcha.ocr_server_port);

    let manager_arc = (*manager).clone();
    // 一次性克隆出 AppState,后续所有 Arc::new 都基于它,避免对 app_state 的多次借用
    let app_state_owned: AppState = app_state.inner().clone();
    let app_state_arc_for_status = Arc::new(app_state_owned);

    {
        let mut cfg = app_state.config.write().await;
        cfg.get_mut().captcha.ocr_server_enabled = true;
        cfg.get_mut().captcha.ocr_server_port = desired_port;
        cfg.save().map_err(|e| format!("保存配置失败: {}", e))?;
    }
    manager_arc.set_enabled(true);

    if !manager_arc.is_running() {
        let manager_for_start = manager_arc.clone();
        manager_for_start
            .start(desired_port, app_state_arc_for_status.clone())
            .await
            .map_err(|e| format!("启动 OCR 服务器失败: {}", e))?;
    }
    ocr_server_status_inner(&manager_arc, &app_state_arc_for_status, Some(desired_port)).await
}

#[tauri::command]
pub async fn ocr_server_stop(
    app_state: State<'_, AppState>,
    manager: State<'_, OcrHttpServerHandle>,
) -> Result<OcrServerStatus, String> {
    let manager_arc = (*manager).clone();

    manager_arc.stop().await;
    manager_arc.set_enabled(false);

    {
        let mut cfg = app_state.config.write().await;
        cfg.get_mut().captcha.ocr_server_enabled = false;
        cfg.save().map_err(|e| format!("保存配置失败: {}", e))?;
    }

    let app_state_owned: AppState = app_state.inner().clone();
    let app_state_arc = Arc::new(app_state_owned);
    ocr_server_status_inner(&manager_arc, &app_state_arc, None).await
}

#[tauri::command]
pub async fn ocr_server_status(
    app_state: State<'_, AppState>,
    manager: State<'_, OcrHttpServerHandle>,
) -> Result<OcrServerStatus, String> {
    let manager_arc = (*manager).clone();
    let app_state_owned: AppState = app_state.inner().clone();
    let app_state_arc = Arc::new(app_state_owned);
    ocr_server_status_inner(&manager_arc, &app_state_arc, None).await
}

#[tauri::command]
pub fn ocr_server_rotate_token(
    manager: State<'_, OcrHttpServerHandle>,
) -> Result<String, String> {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let new_token: String = (0..32)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect();
    let arc: Arc<crate::ocr_server::OcrHttpServerManager> = (*manager).clone();
    // 在独立作用域内 try_lock 并立即写入,避免 guard 与 arc 生命周期冲突
    let written = {
        match arc.token.try_lock() {
            Ok(mut g) => {
                *g = new_token.clone();
                true
            }
            Err(_) => false,
        }
    };
    if written {
        Ok(new_token)
    } else {
        Err("token lock busy, retry".to_string())
    }
}

/// 从 `State<'_, AppState>` 拿到独立 `AppState` 副本。
///
/// `AppState` 本身要求 `Clone`,且其内部字段都是 `Arc<...>`,clone 成本仅递增引用计数。
fn app_state_clone(state: &AppState) -> AppState {
    state.clone()
}

async fn ocr_server_status_inner(
    manager: &Arc<crate::ocr_server::OcrHttpServerManager>,
    app_state: &Arc<AppState>,
    fallback_port: Option<u16>,
) -> Result<OcrServerStatus, String> {
    let cfg_port = app_state.config.read().await.get().captcha.ocr_server_port;
    let port = if manager.is_running() {
        manager.get_port()
    } else {
        fallback_port.unwrap_or(cfg_port)
    };

    let cfg_version = app_state.config.read().await.get().captcha.model_version;
    let (models_loaded, mv) = {
        let guard = app_state
            .local_ocr
            .lock()
            .map_err(|e| format!("local_ocr lock: {}", e))?;
        let loaded = guard.is_some();
        let v = guard.as_ref().map(|b| b.version().as_str().to_string());
        (loaded, v)
    };
    let model_version = mv.or_else(|| Some(cfg_version.as_str().to_string()));

    let token = manager.get_token().await;
    let ips = get_local_ips();
    let url = ips
        .first()
        .map(|ip| format!("http://{}:{}/?token={}", ip, port, token))
        .unwrap_or_else(|| format!("http://127.0.0.1:{}/?token={}", port, token));

    Ok(OcrServerStatus {
        running: manager.is_running(),
        enabled: manager.is_enabled(),
        port,
        token: token.clone(),
        models_loaded,
        model_version,
        total_requests: manager.total_requests(),
        success_count: manager.success_count(),
        failure_count: manager.failure_count(),
        avg_response_ms: manager.avg_response_ms(),
        url,
        ips,
    })
}

fn get_local_ips() -> Vec<String> {
    let mut ips = Vec::new();
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if iface.is_loopback() {
                continue;
            }
            let ip = iface.ip().to_string();
            if !ip.contains(':') {
                ips.push(ip);
            }
        }
    }
    if ips.is_empty() {
        ips.push("127.0.0.1".to_string());
    }
    ips
}
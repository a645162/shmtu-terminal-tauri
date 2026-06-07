use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use shmtu_p2p::{P2PEvent, P2PInfo, P2PSession, P2PStatus};

use crate::export::ExportFormat;
use crate::state::AppState;

/// P2P 服务端信息（前端返回类型）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PInfoFrontend {
    pub port: u16,
    pub pair_code: String,
    pub local_ips: Vec<String>,
    pub qr_payload: String,
    pub is_running: bool,
}

impl From<P2PInfo> for P2PInfoFrontend {
    fn from(info: P2PInfo) -> Self {
        Self {
            port: info.port,
            pair_code: info.pair_code,
            local_ips: info.local_ips,
            qr_payload: info.qr_payload,
            is_running: info.is_running,
        }
    }
}

/// P2P 会话信息（前端返回类型）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PSessionFrontend {
    pub session_id: String,
    pub peer_ip: String,
    pub peer_port: u16,
    pub peer_device_name: String,
    pub is_paired: bool,
    pub is_incoming: bool,
}

impl From<P2PSession> for P2PSessionFrontend {
    fn from(session: P2PSession) -> Self {
        Self {
            session_id: session.session_id,
            peer_ip: session.peer_ip,
            peer_port: session.peer_port,
            peer_device_name: session.peer_device_name,
            is_paired: session.is_paired,
            is_incoming: session.is_incoming,
        }
    }
}

/// P2P 整体状态（前端返回类型）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PStatusFrontend {
    pub server_running: bool,
    pub port: u16,
    pub pair_code: String,
    pub local_ips: Vec<String>,
    pub sessions: Vec<P2PSessionFrontend>,
}

impl From<P2PStatus> for P2PStatusFrontend {
    fn from(status: P2PStatus) -> Self {
        Self {
            server_running: status.server_running,
            port: status.port,
            pair_code: status.pair_code,
            local_ips: status.local_ips,
            sessions: status
                .sessions
                .into_iter()
                .map(P2PSessionFrontend::from)
                .collect(),
        }
    }
}

/// DataReceived 事件载荷（发给前端，data 为 base64 编码）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataReceivedPayload {
    pub session_id: String,
    pub data_base64: String,
}

/// 启动 P2P 服务端
#[tauri::command]
pub async fn p2p_start_server(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<P2PInfoFrontend, String> {
    tracing::info!("[P2P] p2p_start_server");

    let p2p_config = {
        let config = state.config.read().await;
        config.get().p2p.clone()
    };

    let manager = state.p2p_manager.read().await;
    let info = manager.start_server(&p2p_config).await.map_err(|e| {
        tracing::error!("[P2P] Failed to start server: {}", e);
        e.to_string()
    })?;

    // 启动事件转发任务（仅首次取出 rx 时启动）
    let event_rx = manager.take_event_rx().await;
    if let Some(rx) = event_rx {
        let app_handle = app.clone();
        tokio::spawn(async move {
            forward_events(rx, &app_handle).await;
        });
    }

    Ok(P2PInfoFrontend::from(info))
}

/// 停止 P2P 服务端
#[tauri::command]
pub async fn p2p_stop_server(state: State<'_, AppState>) -> Result<(), String> {
    tracing::info!("[P2P] p2p_stop_server");
    let manager = state.p2p_manager.read().await;
    manager.stop_server().await.map_err(|e| {
        tracing::error!("[P2P] Failed to stop server: {}", e);
        e.to_string()
    })
}

/// 获取 QR 码载荷
#[tauri::command]
pub async fn p2p_get_qr_payload(state: State<'_, AppState>) -> Result<String, String> {
    tracing::debug!("[P2P] p2p_get_qr_payload");
    let manager = state.p2p_manager.read().await;
    manager.get_qr_payload().await.map_err(|e| e.to_string())
}

/// 连接到远端设备
#[tauri::command]
pub async fn p2p_connect(
    addr: String,
    port: u16,
    pair_code: String,
    device_name: String,
    state: State<'_, AppState>,
) -> Result<P2PSessionFrontend, String> {
    tracing::info!("[P2P] p2p_connect: {}:{}, code={}", addr, port, pair_code);
    let manager = state.p2p_manager.read().await;
    let session = manager
        .connect(addr, port, pair_code, device_name)
        .await
        .map_err(|e| {
            tracing::error!("[P2P] Failed to connect: {}", e);
            e.to_string()
        })?;

    // 注册会话到 manager
    drop(manager);
    {
        let mgr = state.p2p_manager.write().await;
        mgr.register_session(session.clone()).await;
    }

    Ok(P2PSessionFrontend::from(session))
}

/// 接受配对请求
#[tauri::command]
pub async fn p2p_accept_pairing(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    tracing::info!("[P2P] p2p_accept_pairing: session_id={}", session_id);
    let manager = state.p2p_manager.read().await;
    manager.accept_pairing(&session_id).await.map_err(|e| {
        tracing::error!("[P2P] Failed to accept pairing: {}", e);
        e.to_string()
    })
}

/// 拒绝配对请求
#[tauri::command]
pub async fn p2p_reject_pairing(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    tracing::info!("[P2P] p2p_reject_pairing: session_id={}", session_id);
    let manager = state.p2p_manager.read().await;
    manager.reject_pairing(&session_id).await.map_err(|e| {
        tracing::error!("[P2P] Failed to reject pairing: {}", e);
        e.to_string()
    })
}

/// 发送账单数据
///
/// 使用 ExportService 导出到临时 JSON 文件，读取后通过 P2P 发送
#[tauri::command]
pub async fn p2p_send_bills(
    session_id: String,
    identity_id: Option<i64>,
    date_start: Option<i64>,
    date_end: Option<i64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    tracing::info!(
        "[P2P] p2p_send_bills: session_id={}, identity_id={:?}, date_start={:?}, date_end={:?}",
        session_id,
        identity_id,
        date_start,
        date_end
    );

    // 确定要导出的身份
    let target_identity_id = match identity_id {
        Some(id) => id,
        None => {
            let config = state.config.read().await;
            let default_id = config.get().identity.default_identity_id;
            drop(config);
            if default_id == 0 {
                return Err("没有可用的默认身份".to_string());
            }
            default_id
        }
    };

    // 获取身份名称
    let identity_name = {
        let db = state.db_manager.read().await;
        let identity = db
            .get_identity(target_identity_id)
            .await
            .map_err(|e| e.to_string())?;
        identity
            .as_ref()
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "unknown".to_string())
    };

    // 导出到临时文件
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!(
        "p2p_transfer_{}_{}.json",
        target_identity_id,
        chrono::Utc::now().timestamp_millis()
    ));
    let temp_path_str = temp_path.to_string_lossy().to_string();

    let export_options = crate::export::ExportOptions {
        format: ExportFormat::Json,
        output_path: temp_path_str.clone(),
        include_classification: true,
        start_timestamp: date_start,
        end_timestamp: date_end,
    };

    // 使用 ExportService 执行导出
    {
        let export_service = state.export_service.read().await;
        export_service
            .export_identity_bills(target_identity_id, &identity_name, &export_options)
            .await
            .map_err(|e| {
                tracing::error!("[P2P] Export failed: {}", e);
                e.to_string()
            })?;
    }

    // 读取导出的 JSON 数据
    let json_data = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        std::fs::read(&temp_path_str).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // 清理临时文件
    let cleanup_path = temp_path.clone();
    tokio::task::spawn_blocking(move || {
        let _ = std::fs::remove_file(&cleanup_path);
    });

    tracing::info!(
        "[P2P] Exported {} bytes for identity_id={}",
        json_data.len(),
        target_identity_id
    );

    // 通过 P2P 发送
    let manager = state.p2p_manager.read().await;
    manager
        .send_bills(&session_id, &json_data)
        .await
        .map_err(|e| {
            tracing::error!("[P2P] Failed to send bills: {}", e);
            e.to_string()
        })
}

/// 获取 P2P 状态
#[tauri::command]
pub async fn p2p_get_status(state: State<'_, AppState>) -> Result<P2PStatusFrontend, String> {
    tracing::debug!("[P2P] p2p_get_status");
    let manager = state.p2p_manager.read().await;
    let status = manager.get_status().await;
    Ok(P2PStatusFrontend::from(status))
}

/// 断开会话
#[tauri::command]
pub async fn p2p_disconnect(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    tracing::info!("[P2P] p2p_disconnect: session_id={}", session_id);
    let manager = state.p2p_manager.read().await;
    manager.disconnect(&session_id).await.map_err(|e| {
        tracing::error!("[P2P] Failed to disconnect: {}", e);
        e.to_string()
    })?;

    // 从管理器移除会话
    drop(manager);
    {
        let mgr = state.p2p_manager.write().await;
        mgr.remove_session(&session_id).await;
    }

    Ok(())
}

/// 手动配对（等价于 connect）
#[tauri::command]
pub async fn p2p_manual_pair(
    ip: String,
    port: u16,
    pair_code: String,
    state: State<'_, AppState>,
) -> Result<P2PSessionFrontend, String> {
    tracing::info!("[P2P] p2p_manual_pair: {}:{}, code={}", ip, port, pair_code);

    // 从配置获取 device_name
    let device_name = {
        let config = state.config.read().await;
        config.get().p2p.device_name.clone()
    };

    let manager = state.p2p_manager.read().await;
    let session = manager
        .manual_pair(ip, port, pair_code, device_name)
        .await
        .map_err(|e| {
            tracing::error!("[P2P] Failed to manual pair: {}", e);
            e.to_string()
        })?;

    // 注册会话到 manager
    drop(manager);
    {
        let mgr = state.p2p_manager.write().await;
        mgr.register_session(session.clone()).await;
    }

    Ok(P2PSessionFrontend::from(session))
}

/// 将 P2P 事件转发到 Tauri 前端
async fn forward_events(mut rx: tokio::sync::mpsc::UnboundedReceiver<P2PEvent>, app: &AppHandle) {
    while let Some(event) = rx.recv().await {
        match event {
            P2PEvent::PairingRequest(req) => {
                let _ = app.emit("p2p-pairing-request", &req);
            }
            P2PEvent::TransferProgress(progress) => {
                let _ = app.emit("p2p-transfer-progress", &progress);
            }
            P2PEvent::TransferComplete(complete) => {
                let _ = app.emit("p2p-transfer-complete", &complete);
            }
            P2PEvent::TransferError(error) => {
                let _ = app.emit("p2p-transfer-error", &error);
            }
            P2PEvent::DataReceived(data_received) => {
                // 将数据 base64 编码后发送给前端
                let payload = DataReceivedPayload {
                    session_id: data_received.session_id.clone(),
                    data_base64: base64::engine::general_purpose::STANDARD
                        .encode(&data_received.data),
                };
                let _ = app.emit("p2p-data-received", &payload);
            }
        }
    }
    tracing::debug!("[P2P] Event forwarding loop ended");
}

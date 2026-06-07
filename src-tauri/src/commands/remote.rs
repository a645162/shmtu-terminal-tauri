//! Tauri commands for RESTful remote access

use crate::remote::{RemoteManager, RemoteSession};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteSessionFrontend {
    pub session_id: String,
    pub base_url: String,
    pub device_name: String,
    pub has_token: bool,
}

impl From<RemoteSession> for RemoteSessionFrontend {
    fn from(s: RemoteSession) -> Self {
        Self {
            session_id: s.session_id,
            base_url: s.base_url,
            device_name: s.device_name,
            has_token: true, // 始终有 token，UI 可选隐藏显示
        }
    }
}

#[tauri::command]
pub async fn remote_connect(
    base_url: String,
    device_name: String,
    manager: State<'_, RemoteManager>,
) -> Result<RemoteSessionFrontend, String> {
    tracing::info!("[Remote] remote_connect: base_url={}, device_name={}", base_url, device_name);
    manager
        .connect(base_url, device_name)
        .await
        .map(RemoteSessionFrontend::from)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_disconnect(
    session_id: String,
    manager: State<'_, RemoteManager>,
) -> Result<(), String> {
    manager.disconnect(&session_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_list_sessions(
    manager: State<'_, RemoteManager>,
) -> Result<Vec<RemoteSessionFrontend>, String> {
    Ok(manager
        .list_sessions()
        .await
        .into_iter()
        .map(RemoteSessionFrontend::from)
        .collect())
}

#[tauri::command]
pub async fn remote_list_identities(
    session_id: String,
    manager: State<'_, RemoteManager>,
) -> Result<Vec<serde_json::Value>, String> {
    manager
        .list_identities(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_list_bills(
    session_id: String,
    query: HashMap<String, String>,
    manager: State<'_, RemoteManager>,
) -> Result<Vec<serde_json::Value>, String> {
    manager
        .list_bills(&session_id, &query)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_export(
    session_id: String,
    manager: State<'_, RemoteManager>,
) -> Result<String, String> {
    manager
        .export_json(&session_id)
        .await
        .map_err(|e| e.to_string())
}

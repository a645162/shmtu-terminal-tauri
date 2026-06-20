//! Tauri commands for P2P RESTful server & client (aligned with Android P2PHttpServer/P2PManager)

use tauri::State;

use crate::p2p::http_server::{
    P2PClient, P2PRestDiscoverData, P2PRestPairRequest, P2PRestPairResponseData,
    P2PRestTransferResponseData,
};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct P2PServerStatus {
    pub running: bool,
    pub port: u16,
    pub device_name: String,
    pub pair_code: String,
}

#[tauri::command]
pub async fn p2p_get_status(state: State<'_, AppState>) -> Result<P2PServerStatus, String> {
    let port = state.p2p_server.get_port();
    let pair_code = state.p2p_server.current_pair_code.read().await.clone();
    Ok(P2PServerStatus {
        running: state.p2p_server.is_running(),
        port,
        device_name: state.p2p_server.device_name.clone(),
        pair_code,
    })
}

#[tauri::command]
pub async fn p2p_start_server(
    state: State<'_, AppState>,
    port: u16,
) -> Result<(), String> {
    state
        .p2p_server
        .clone()
        .start(port, state.db_manager.clone(), state.config.clone())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn p2p_stop_server(state: State<'_, AppState>) -> Result<(), String> {
    state.p2p_server.stop().await;
    Ok(())
}

#[tauri::command]
pub async fn p2p_set_pair_code(
    state: State<'_, AppState>,
    code: String,
) -> Result<(), String> {
    state.p2p_server.set_pair_code(code);
    Ok(())
}

#[tauri::command]
pub async fn p2p_discover(
    base_url: String,
    device_name: String,
) -> Result<P2PRestDiscoverData, String> {
    let client = P2PClient::new();
    client.discover(&base_url, &device_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn p2p_pair(
    base_url: String,
    pair_code: String,
    device_name: String,
    listen_port: u16,
    listen_ips: Vec<String>,
) -> Result<P2PRestPairResponseData, String> {
    let client = P2PClient::new();
    let req = P2PRestPairRequest { pair_code, device_name, listen_port, listen_ips };
    client.pair(&base_url, req).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn p2p_upload_transfer(
    base_url: String,
    peer_key: String,
    session_id: String,
    bill_count: i32,
    zip_data: Vec<u8>,
) -> Result<P2PRestTransferResponseData, String> {
    let client = P2PClient::new();
    client
        .upload_transfer(&base_url, &peer_key, &session_id, bill_count, zip_data)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn p2p_download_transfer(
    base_url: String,
    peer_key: String,
    session_id: String,
) -> Result<Vec<u8>, String> {
    let client = P2PClient::new();
    client
        .download_transfer(&base_url, &peer_key, &session_id)
        .await
        .map_err(|e| e.to_string())
}

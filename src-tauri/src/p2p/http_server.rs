//! Tauri 端 P2P RESTful HTTP 服务器
//!
//! 与 Android 端 P2PHttpServer 协议完全对齐：
//! - POST /api/p2p/discover — 设备发现（无需认证）
//! - POST /api/p2p/pair — 配对（验证 pairCode，返回 peerKey）
//! - POST /api/p2p/transfer — 上传加密 ZIP（需 P2P-Key 认证）
//! - GET  /api/p2p/transfer/{sessionId} — 下载 ZIP（需 P2P-Key 认证）
//!
//! 认证：Authorization: P2P-Key <peerKey>

use anyhow::{anyhow, Result};
use axum::{
    body::Bytes,
    extract::{Multipart, Path, State as AxState},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::config::TomlConfig;
use crate::db::DatabaseManager;
use crate::entity::{accounts, bill_merged, identities};

// ============================================================================
// 协议数据模型（与 Android P2PRestDiscoverData 等一一对应）
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PRestDiscoverRequest {
    #[serde(default)]
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PRestDiscoverData {
    pub device_name: String,
    pub ips: Vec<String>,
    pub port: u16,
    pub pair_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PRestPairRequest {
    pub pair_code: String,
    pub device_name: String,
    pub listen_port: u16,
    #[serde(default)]
    pub listen_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PRestPairResponseData {
    pub session_id: String,
    pub device_name: String,
    pub peer_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PRestTransferResponseData {
    pub received: bool,
    pub bill_count: i32,
    pub checksum: String,
}

// ============================================================================
// P2P 会话存储
// ============================================================================

#[derive(Debug, Clone)]
pub struct P2PHTTPSession {
    pub session_id: String,
    pub peer_key: String,
    pub remote_device_name: String,
    pub remote_ips: Vec<String>,
    pub remote_port: u16,
    pub pair_code: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PTransferEvent {
    pub session_id: String,
    pub device_name: String,
    pub data: Vec<u8>,
    pub bill_count: i32,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PPairEvent {
    pub session_id: String,
    pub remote_device_name: String,
    pub remote_ips: Vec<String>,
    pub remote_port: u16,
    pub peer_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }
}

pub fn api_error(msg: impl Into<String>) -> ApiResponse<()> {
    ApiResponse { success: false, data: None, error: Some(msg.into()) }
}

// ============================================================================
// P2P HTTP 服务器管理器
// ============================================================================

pub struct P2PServerManager {
    pub device_name: String,
    pub current_pair_code: RwLock<String>,
    sessions: RwLock<HashMap<String, P2PHTTPSession>>,
    pair_tx: broadcast::Sender<P2PPairEvent>,
    transfer_tx: broadcast::Sender<P2PTransferEvent>,
    server_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    running_port: RwLock<u16>,
}

impl P2PServerManager {
    pub fn new(device_name: String) -> Arc<Self> {
        let (pair_tx, _) = broadcast::channel(16);
        let (transfer_tx, _) = broadcast::channel(16);
        Arc::new(Self {
            device_name,
            current_pair_code: RwLock::new(generate_pair_code()),
            sessions: RwLock::new(HashMap::new()),
            pair_tx,
            transfer_tx,
            server_handle: RwLock::new(None),
            running_port: RwLock::new(0),
        })
    }

    pub fn set_pair_code(&self, code: String) {
        if let Ok(mut c) = self.current_pair_code.try_write() {
            *c = code;
        }
    }

    pub fn is_running(&self) -> bool {
        self.running_port.try_read().map(|p| *p > 0).unwrap_or(false)
    }

    pub fn get_port(&self) -> u16 {
        self.running_port.try_read().map(|p| *p).unwrap_or(0)
    }

    pub fn subscribe_pairs(&self) -> broadcast::Receiver<P2PPairEvent> {
        self.pair_tx.subscribe()
    }

    pub fn subscribe_transfers(&self) -> broadcast::Receiver<P2PTransferEvent> {
        self.transfer_tx.subscribe()
    }

    pub async fn start(
        self: Arc<Self>,
        port: u16,
        db_manager: Arc<RwLock<DatabaseManager>>,
        config: Arc<RwLock<TomlConfig>>,
    ) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        let manager = self.clone();
        let db_for_state = db_manager.clone();
        let cfg_for_state = config.clone();
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let router = build_router(manager, db_for_state, cfg_for_state);

        let handle = tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("[P2PServer] bind error: {}", e);
                    return;
                }
            };
            tracing::info!("[P2PServer] listening on {}", addr);
            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!("[P2PServer] serve error: {}", e);
            }
        });

        *self.running_port.write().await = port;
        *self.server_handle.write().await = Some(handle);
        Ok(())
    }

    pub async fn stop(&self) {
        if let Some(handle) = self.server_handle.write().await.take() {
            handle.abort();
        }
        *self.running_port.write().await = 0;
        self.sessions.write().await.clear();
    }
}

// ============================================================================
// Router 构建
// ============================================================================

fn build_router(
    manager: Arc<P2PServerManager>,
    db_manager: Arc<RwLock<DatabaseManager>>,
    config: Arc<RwLock<TomlConfig>>,
) -> Router {
    let state = (manager, db_manager, config);
    Router::new()
        .route("/api/p2p/discover", post(handle_discover))
        .route("/api/p2p/pair", post(handle_pair))
        .route("/api/p2p/transfer", post(handle_transfer_upload))
        .route("/api/p2p/transfer/{session_id}", get(handle_transfer_download))
        .with_state(state)
}

type AppState = (Arc<P2PServerManager>, Arc<RwLock<DatabaseManager>>, Arc<RwLock<TomlConfig>>);

async fn handle_discover(
    AxState((manager, _, _)): AxState<AppState>,
    Json(req): Json<P2PRestDiscoverRequest>,
) -> Response {
    let port = manager.get_port();
    let pair_code = manager.current_pair_code.read().await.clone();
    let ips = get_local_ips();
    let data = P2PRestDiscoverData {
        device_name: manager.device_name.clone(),
        ips,
        port,
        pair_code,
    };
    tracing::info!(
        "[P2PServer] discover from '{}', returning name={} port={}",
        req.device_name, data.device_name, port
    );
    Json(ApiResponse::success(data)).into_response()
}

async fn handle_pair(
    AxState((manager, _, _)): AxState<AppState>,
    Json(req): Json<P2PRestPairRequest>,
) -> Response {
    if req.pair_code.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "pair_code is required");
    }
    let current = manager.current_pair_code.read().await.clone();
    if !req.pair_code.eq_ignore_ascii_case(&current) {
        tracing::warn!("[P2PServer] pair rejected: invalid pair_code from '{}'", req.device_name);
        return json_error(StatusCode::FORBIDDEN, "配对码不正确");
    }

    let session_id = Uuid::new_v4().to_string();
    let peer_key = generate_peer_key();
    let session = P2PHTTPSession {
        session_id: session_id.clone(),
        peer_key: peer_key.clone(),
        remote_device_name: req.device_name.clone(),
        remote_ips: req.listen_ips.clone(),
        remote_port: req.listen_port,
        pair_code: req.pair_code.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    manager.sessions.write().await.insert(peer_key.clone(), session);

    let data = P2PRestPairResponseData {
        session_id,
        device_name: manager.device_name.clone(),
        peer_key,
    };
    tracing::info!("[P2PServer] pair accepted from '{}'", req.device_name);

    let _ = manager.pair_tx.send(P2PPairEvent {
        session_id: data.session_id.clone(),
        remote_device_name: req.device_name,
        remote_ips: req.listen_ips,
        remote_port: req.listen_port,
        peer_key: data.peer_key.clone(),
    });

    Json(ApiResponse::success(data)).into_response()
}

async fn handle_transfer_upload(
    AxState((manager, _, _)): AxState<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let peer_key = match extract_p2p_key(&headers) {
        Some(k) => k,
        None => return json_error(StatusCode::UNAUTHORIZED, "Unauthorized - P2P-Key missing"),
    };
    if !manager.sessions.read().await.contains_key(&peer_key) {
        return json_error(StatusCode::UNAUTHORIZED, "Unauthorized - P2P-Key invalid");
    }

    let mut file_data: Option<Bytes> = None;
    let mut session_id = String::new();
    let mut bill_count: i32 = 0;

    while let Some(field) = match multipart.next_field().await {
        Ok(f) => f,
        Err(e) => return json_error(StatusCode::BAD_REQUEST, &format!("multipart error: {}", e)),
    } {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => match field.bytes().await {
                Ok(b) => file_data = Some(b),
                Err(e) => return json_error(StatusCode::BAD_REQUEST, &format!("file read: {}", e)),
            },
            "sessionId" => {
                if let Ok(t) = field.text().await {
                    session_id = t;
                }
            }
            "billCount" => {
                if let Ok(t) = field.text().await {
                    bill_count = t.parse().unwrap_or(0);
                }
            }
            _ => {}
        }
    }

    let data = match file_data {
        Some(d) if !d.is_empty() => d,
        _ => return json_error(StatusCode::BAD_REQUEST, "file is required"),
    };

    let device_name = manager
        .sessions
        .read()
        .await
        .get(&peer_key)
        .map(|s| s.remote_device_name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let checksum = short_sha256(&data);
    tracing::info!(
        "[P2PServer] upload session={} bytes={} billCount={} checksum={}",
        session_id, data.len(), bill_count, checksum
    );

    let _ = manager.transfer_tx.send(P2PTransferEvent {
        session_id: session_id.clone(),
        device_name,
        data: data.to_vec(),
        bill_count,
        checksum: checksum.clone(),
    });

    let resp = P2PRestTransferResponseData { received: true, bill_count, checksum };
    Json(ApiResponse::success(resp)).into_response()
}

async fn handle_transfer_download(
    AxState((manager, db_lock, _)): AxState<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Response {
    let peer_key = match extract_p2p_key(&headers) {
        Some(k) => k,
        None => return json_error(StatusCode::UNAUTHORIZED, "Unauthorized - P2P-Key missing"),
    };
    let session_exists = manager
        .sessions
        .read()
        .await
        .values()
        .any(|s| s.session_id == session_id);
    if !session_exists {
        return json_error(StatusCode::NOT_FOUND, &format!("Pair session not found: {}", session_id));
    }

    // 短期取出 DatabaseManager clone（DatabaseManager: Clone），避免跨 await 持有锁
    let db_manager = db_lock.read().await.clone();

    let archive = match build_p2p_archive(&db_manager).await {
        Ok(b) => b,
        Err(e) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &format!("export failed: {}", e)),
    };

    if archive.is_empty() {
        return json_error(StatusCode::NOT_FOUND, "No bill data available for export");
    }

    tracing::info!("[P2PServer] download session={} bytes={}", session_id, archive.len());
    (
        StatusCode::OK,
        [("content-type", "application/octet-stream")],
        archive,
    ).into_response()
}

// ============================================================================
// 工具函数
// ============================================================================

fn extract_p2p_key(headers: &HeaderMap) -> Option<String> {
    let h = headers.get("authorization")?.to_str().ok()?;
    if h.len() > 8 && h[..8].eq_ignore_ascii_case("P2P-Key ") {
        let k = h[8..].trim();
        if k.is_empty() { None } else { Some(k.to_string()) }
    } else {
        None
    }
}

fn json_error(status: StatusCode, msg: &str) -> Response {
    (status, Json(api_error(msg))).into_response()
}

const KEY_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn generate_pair_code() -> String {
    let mut rng = rand::thread_rng();
    (0..6).map(|_| KEY_CHARS[rng.gen_range(0..KEY_CHARS.len())] as char).collect()
}

fn generate_peer_key() -> String {
    let mut rng = rand::thread_rng();
    (0..32).map(|_| KEY_CHARS[rng.gen_range(0..KEY_CHARS.len())] as char).collect()
}

fn short_sha256(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut s = String::with_capacity(16);
    for b in digest.iter().take(8) {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn get_local_ips() -> Vec<String> {
    let mut ips = Vec::new();
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if iface.is_loopback() { continue; }
            let ip = iface.ip().to_string();
            if !ip.contains(':') {
                ips.push(ip);
            }
        }
    }
    if ips.is_empty() { ips.push("127.0.0.1".to_string()); }
    ips
}

// ============================================================================
// 归档构建（与 Android TransferArchiveService 对齐）
// ============================================================================

use sea_orm::{ColumnTrait, Condition, EntityTrait};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransferArchive {
    schema_version: u32,
    export_time: i64,
    format: String,
    source_platform: String,
    identities: Vec<IdentityBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IdentityBundle {
    identity: IdentityInfo,
    accounts: Vec<AccountBundle>,
    bills: Vec<BillInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IdentityInfo {
    name: String,
    enable: bool,
    birthday: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountBundle {
    entity: AccountInfo,
    password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountInfo {
    account_name: String,
    account_id: String,
    enable: bool,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BillInfo {
    date_time_formatted: Option<String>,
    item_type: Option<String>,
    number: Option<String>,
    target_user: Option<String>,
    money_str: Option<String>,
    money: Option<f64>,
    method: Option<String>,
    status_str: Option<String>,
    is_combined: bool,
    source_account_id: Option<String>,
    position: Option<String>,
    category: Option<String>,
    notes: Option<String>,
}

async fn build_p2p_archive(db_manager: &DatabaseManager) -> Result<Vec<u8>> {
    let db = db_manager.db().clone();

    let identity_models = identities::Entity::find().all(&db).await?;
    let acc_models_all = accounts::Entity::find().all(&db).await?;
    let bill_models_all = bill_merged::Entity::find().all(&db).await?;

    let mut bundles = Vec::new();
    let mut total_bills = 0;

    for im in identity_models {
        let acc_bundles: Vec<AccountBundle> = acc_models_all
            .iter()
            .filter(|a| a.identity_id == im.id)
            .map(|a| AccountBundle {
                entity: AccountInfo {
                    account_name: a.account_name.clone(),
                    account_id: a.account_id.clone(),
                    enable: a.enable,
                    created_at: a.created_at.clone(),
                },
                password: a.password.clone(),
            })
            .collect();

        let bill_infos: Vec<BillInfo> = bill_models_all
            .iter()
            .filter(|b| b.identity_id == im.id)
            .map(|b| BillInfo {
                date_time_formatted: b.date_time_formatted.clone(),
                item_type: b.item_type.clone(),
                number: b.number.clone(),
                target_user: b.target_user.clone(),
                money_str: b.money_str.clone(),
                money: b.money,
                method: b.method.clone(),
                status_str: b.status_str.clone(),
                is_combined: b.is_combined,
                source_account_id: b.source_account_id.clone(),
                position: b.position.clone(),
                category: b.category.clone(),
                notes: b.notes.clone(),
            })
            .collect();
        total_bills += bill_infos.len();

        bundles.push(IdentityBundle {
            identity: IdentityInfo {
                name: im.name.clone(),
                enable: im.enable,
                birthday: im.birthday.clone(),
                created_at: im.created_at.clone(),
            },
            accounts: acc_bundles,
            bills: bill_infos,
        });
    }

    let archive = TransferArchive {
        schema_version: 2,
        export_time: chrono::Utc::now().timestamp_millis(),
        format: "shmtu-transfer-archive".to_string(),
        source_platform: "tauri".to_string(),
        identities: bundles,
    };

    let json_str = serde_json::to_string_pretty(&archive)?;
    let mut zip_buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut zip_buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", options)?;
        std::io::Write::write_all(&mut zip, json_str.as_bytes())?;
        zip.finish()?;
    }
    tracing::info!("[P2PServer] built archive: {} identities, {} bills",
        archive.identities.len(), total_bills);
    Ok(zip_buf.into_inner())
}

// ============================================================================
// P2P 客户端（与 Android P2PManager 协议对齐）
// ============================================================================

pub struct P2PClient {
    pub http: reqwest::Client,
}

impl P2PClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn discover(&self, base_url: &str, device_name: &str) -> Result<P2PRestDiscoverData> {
        let url = format!("{}/api/p2p/discover", base_url.trim_end_matches('/'));
        let resp = self.http.post(&url)
            .json(&P2PRestDiscoverRequest { device_name: device_name.to_string() })
            .send().await
            .map_err(|e| anyhow!("discover: {}", e))?;
        if !resp.status().is_success() {
            return Err(anyhow!("discover HTTP {}", resp.status()));
        }
        let body: ApiResponse<P2PRestDiscoverData> = resp.json().await
            .map_err(|e| anyhow!("discover parse: {}", e))?;
        body.data.ok_or_else(|| anyhow!("discover: missing data"))
    }

    pub async fn pair(&self, base_url: &str, req: P2PRestPairRequest) -> Result<P2PRestPairResponseData> {
        let url = format!("{}/api/p2p/pair", base_url.trim_end_matches('/'));
        let resp = self.http.post(&url)
            .json(&req)
            .send().await
            .map_err(|e| anyhow!("pair: {}", e))?;
        if !resp.status().is_success() {
            return Err(anyhow!("pair HTTP {}", resp.status()));
        }
        let body: ApiResponse<P2PRestPairResponseData> = resp.json().await
            .map_err(|e| anyhow!("pair parse: {}", e))?;
        body.data.ok_or_else(|| anyhow!("pair: missing data"))
    }

    pub async fn upload_transfer(
        &self, base_url: &str, peer_key: &str,
        session_id: &str, bill_count: i32, zip_data: Vec<u8>,
    ) -> Result<P2PRestTransferResponseData> {
        let url = format!("{}/api/p2p/transfer", base_url.trim_end_matches('/'));
        let part = reqwest::multipart::Part::bytes(zip_data).file_name("transfer.zip");
        let form = reqwest::multipart::Form::new()
            .text("sessionId", session_id.to_string())
            .text("billCount", bill_count.to_string())
            .part("file", part);
        let resp = self.http.post(&url)
            .header("Authorization", format!("P2P-Key {}", peer_key))
            .multipart(form)
            .send().await
            .map_err(|e| anyhow!("upload: {}", e))?;
        if !resp.status().is_success() {
            return Err(anyhow!("upload HTTP {}", resp.status()));
        }
        let body: ApiResponse<P2PRestTransferResponseData> = resp.json().await
            .map_err(|e| anyhow!("upload parse: {}", e))?;
        body.data.ok_or_else(|| anyhow!("upload: missing data"))
    }

    pub async fn download_transfer(&self, base_url: &str, peer_key: &str, session_id: &str) -> Result<Vec<u8>> {
        let url = format!("{}/api/p2p/transfer/{}", base_url.trim_end_matches('/'), session_id);
        let resp = self.http.get(&url)
            .header("Authorization", format!("P2P-Key {}", peer_key))
            .send().await
            .map_err(|e| anyhow!("download: {}", e))?;
        if !resp.status().is_success() {
            return Err(anyhow!("download HTTP {}", resp.status()));
        }
        Ok(resp.bytes().await?.to_vec())
    }
}

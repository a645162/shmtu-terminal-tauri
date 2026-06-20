//! Tauri 端 RESTful OCR 推理服务器
//!
//! 把 AppState 中的 `local_ocr: Arc<std::sync::Mutex<Option<OcrBackend>>>` 通过 HTTP 暴露给
//! 同网段的 Android/Tauri/C# 等客户端复用本地 ONNX 推理能力。
//!
//! **懒加载**: 服务启动时不加载模型,首次 `POST /api/ocr` 收到请求时才在 `spawn_blocking`
//! 里调 `OcrBackend::load`。已加载后所有请求复用同一 backend 实例。
//!
//! **协议对齐**: 与 Android 端 `OcrWebServer` 完全一致 —
//! 请求 `{"imageBase64": "..."}`,响应 `{"success": true, "expression": "3+5=8", "result": 8}`
//! 或 `{"success": false, "error": "..."}`。`/api/health` 返回模型加载状态。

use anyhow::{anyhow, Result};
use axum::{
    extract::State as AxState,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use shmtu_ocr::backend::OcrBackend;
use shmtu_ocr::ModelVersion;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use tokio::sync::Mutex as TokioMutex;

use crate::config::TomlConfig;
use crate::state::AppState as CrateAppState;

// ============================================================================
// 协议数据模型
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct OcrRequest {
    #[serde(default)]
    pub image_base64: String,
    #[serde(default)]
    pub imageBase64: String,
    /// 可选: 覆盖服务端默认模型版本 (v1 / v2)。
    #[serde(default, alias = "modelVersion")]
    pub model_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OcrResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl OcrResponse {
    pub fn success(expr: String, result: i32, mv: ModelVersion, ms: u128) -> Self {
        Self {
            success: true,
            expression: Some(expr),
            result: Some(result),
            model_version: Some(mv.as_str().to_string()),
            duration_ms: Some(ms),
            error: None,
        }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            expression: None,
            result: None,
            model_version: None,
            duration_ms: None,
            error: Some(msg.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub models_loaded: bool,
    pub model_version: Option<String>,
    pub server: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub models_loaded: bool,
    pub model_version: Option<String>,
    pub total_requests: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_response_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InfoResponse {
    pub device_name: String,
    pub ips: Vec<String>,
    pub port: u16,
    pub token: String,
    pub protocol_version: String,
    pub server: String,
}

// ============================================================================
// 服务器管理器
// ============================================================================

/// Tauri 端 OCR HTTP 服务器管理器
///
/// 持有 [CrateAppState] 的 Arc 引用,通过 axum router 暴露端点。
/// 模型懒加载:`local_ocr` 字段从 `None` 变为 `Some(backend)` 仅在首次
/// `POST /api/ocr` 期间发生。
pub struct OcrHttpServerManager {
    pub device_name: String,
    running_port: RwLock<u16>,
    server_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    pub enabled: AtomicBool,
    pub token: TokioMutex<String>,
    total_requests: AtomicU64,
    success_count: AtomicU64,
    failure_count: AtomicU64,
    total_response_ms: AtomicU64,
}

impl OcrHttpServerManager {
    pub fn new(device_name: String) -> Arc<Self> {
        Arc::new(Self {
            device_name,
            running_port: RwLock::new(0),
            server_handle: RwLock::new(None),
            enabled: AtomicBool::new(false),
            token: TokioMutex::new(generate_token()),
            total_requests: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            total_response_ms: AtomicU64::new(0),
        })
    }

    pub fn is_running(&self) -> bool {
        self.running_port.try_read().map(|p| *p > 0).unwrap_or(false)
    }

    pub fn get_port(&self) -> u16 {
        self.running_port.try_read().map(|p| *p).unwrap_or(0)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub async fn get_token(&self) -> String {
        self.token.lock().await.clone()
    }

    pub async fn set_token(&self, new_token: String) {
        *self.token.lock().await = new_token;
    }

    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }
    pub fn success_count(&self) -> u64 {
        self.success_count.load(Ordering::Relaxed)
    }
    pub fn failure_count(&self) -> u64 {
        self.failure_count.load(Ordering::Relaxed)
    }
    pub fn avg_response_ms(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        self.total_response_ms.load(Ordering::Relaxed) as f64 / total as f64
    }

    /// 启动 HTTP 服务器 (懒加载:不加载模型)
    pub async fn start(
        self: Arc<Self>,
        port: u16,
        app_state: Arc<CrateAppState>,
    ) -> Result<()> {
        if self.is_running() {
            tracing::warn!("[OcrHttpServer] already running on port {}", self.get_port());
            return Ok(());
        }
        let manager = self.clone();
        let state_for_handler = app_state.clone();
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let router = build_router(manager, state_for_handler);

        let handle = tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("[OcrHttpServer] bind error: {}", e);
                    return;
                }
            };
            tracing::info!("[OcrHttpServer] listening on {} (lazy-load model)", addr);
            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!("[OcrHttpServer] serve error: {}", e);
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
        tracing::info!("[OcrHttpServer] stopped");
    }
}

// ============================================================================
// Router + 处理器
// ============================================================================

fn build_router(
    manager: Arc<OcrHttpServerManager>,
    app_state: Arc<CrateAppState>,
) -> Router {
    Router::new()
        .route("/api/health", get(handle_health))
        .route("/api/info", get(handle_info))
        .route("/api/status", get(handle_status))
        .route("/api/ocr", post(handle_ocr))
        .with_state((manager, app_state))
}

type SharedState = (Arc<OcrHttpServerManager>, Arc<CrateAppState>);

async fn handle_health(AxState((manager, app_state)): AxState<SharedState>) -> Response {
    let loaded = {
        let guard = app_state.local_ocr.lock().expect("local_ocr mutex poisoned");
        guard.is_some()
    };
    let payload = HealthResponse {
        status: if loaded { "healthy".into() } else { "loading".into() },
        models_loaded: loaded,
        model_version: current_loaded_version_string(&app_state).await,
        server: "tauri-shmtu-ocr".into(),
    };
    Json(payload).into_response()
}

async fn handle_info(AxState((manager, _)): AxState<SharedState>) -> Response {
    let token = manager.get_token().await;
    let payload = InfoResponse {
        device_name: manager.device_name.clone(),
        ips: get_local_ips(),
        port: manager.get_port(),
        token,
        protocol_version: "1.0".into(),
        server: "tauri-shmtu-ocr".into(),
    };
    Json(payload).into_response()
}

async fn handle_status(AxState((manager, app_state)): AxState<SharedState>) -> Response {
    let loaded = {
        let guard = app_state.local_ocr.lock().expect("local_ocr mutex poisoned");
        guard.is_some()
    };
    let payload = StatusResponse {
        status: if loaded { "healthy".into() } else { "loading".into() },
        models_loaded: loaded,
        model_version: current_loaded_version_string(&app_state).await,
        total_requests: manager.total_requests(),
        success_count: manager.success_count(),
        failure_count: manager.failure_count(),
        avg_response_ms: manager.avg_response_ms(),
    };
    Json(payload).into_response()
}

async fn current_loaded_version_string(app_state: &Arc<CrateAppState>) -> Option<String> {
    let guard = app_state.local_ocr.lock().expect("local_ocr mutex poisoned");
    guard.as_ref().map(|b| b.version().as_str().to_string())
}

async fn handle_ocr(
    AxState((manager, app_state)): AxState<SharedState>,
    headers: HeaderMap,
    Json(req): Json<OcrRequest>,
) -> Response {
    let start = Instant::now();
    manager.total_requests.fetch_add(1, Ordering::Relaxed);

    // 鉴权
    let expected = manager.get_token().await;
    if !is_authorized(&headers, &expected) {
        manager.failure_count.fetch_add(1, Ordering::Relaxed);
        return json_response(
            StatusCode::UNAUTHORIZED,
            OcrResponse::error("Unauthorized - token invalid or missing"),
        );
    }

    let image_b64 = if !req.image_base64.is_empty() {
        req.image_base64.clone()
    } else if !req.imageBase64.is_empty() {
        req.imageBase64.clone()
    } else {
        manager.failure_count.fetch_add(1, Ordering::Relaxed);
        return json_response(
            StatusCode::BAD_REQUEST,
            OcrResponse::error("image_base64 / imageBase64 is empty"),
        );
    };

    let bytes = match BASE64.decode(image_b64.trim()) {
        Ok(b) => b,
        Err(e) => {
            manager.failure_count.fetch_add(1, Ordering::Relaxed);
            return json_response(
                StatusCode::BAD_REQUEST,
                OcrResponse::error(format!("base64 decode: {}", e)),
            );
        }
    };
    if bytes.is_empty() {
        manager.failure_count.fetch_add(1, Ordering::Relaxed);
        return json_response(StatusCode::BAD_REQUEST, OcrResponse::error("image bytes empty"));
    }

    let requested_version = req
        .model_version
        .as_deref()
        .map(ModelVersion::parse_or_default);

    let load_outcome = ensure_loaded_lazy(app_state.clone(), requested_version).await;
    let loaded_version = match load_outcome {
        Ok(v) => v,
        Err(e) => {
            manager.failure_count.fetch_add(1, Ordering::Relaxed);
            return json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                OcrResponse::error(format!("model load failed: {}", e)),
            );
        }
    };

    let app_state_for_blocking = app_state.clone();
    let predict_outcome: Result<(String, i32)> = tokio::task::spawn_blocking(move || {
        let mut guard = app_state_for_blocking
            .local_ocr
            .lock()
            .expect("local_ocr mutex poisoned");
        let backend = guard
            .as_mut()
            .ok_or_else(|| anyhow!("local OCR not initialized"))?;
        let r = backend
            .predict_bytes(&bytes)
            .map_err(|e| anyhow!("predict failed: {}", e))?;
        let answer = shmtu_cas::captcha::get_expr_result(&r.expr);
        let answer_int = answer.parse::<i32>().unwrap_or(0);
        Ok((r.expr, answer_int))
    })
    .await
    .map_err(|e| anyhow!("predict task join: {}", e))
    .and_then(|inner| inner);

    let elapsed = start.elapsed().as_millis();
    manager
        .total_response_ms
        .fetch_add(elapsed as u64, Ordering::Relaxed);

    match predict_outcome {
        Ok((expr, answer)) => {
            manager.success_count.fetch_add(1, Ordering::Relaxed);
            tracing::info!(
                "[OcrHttpServer] OCR ok expr={} answer={} ({}ms, {})",
                expr,
                answer,
                elapsed,
                loaded_version.as_str()
            );
            json_response(
                StatusCode::OK,
                OcrResponse::success(expr, answer, loaded_version, elapsed),
            )
        }
        Err(e) => {
            manager.failure_count.fetch_add(1, Ordering::Relaxed);
            tracing::warn!("[OcrHttpServer] OCR failed: {}", e);
            json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                OcrResponse::error(e.to_string()),
            )
        }
    }
}

// ============================================================================
// 懒加载辅助
// ============================================================================

/// 懒加载模型:仅当 `local_ocr` 为 None 或版本不一致时才加载。
async fn ensure_loaded_lazy(
    app_state: Arc<CrateAppState>,
    requested_version: Option<ModelVersion>,
) -> Result<ModelVersion> {
    // 读路径:快速检查
    {
        let guard = app_state.local_ocr.lock().expect("local_ocr mutex poisoned");
        if let Some(backend) = guard.as_ref() {
            let current = backend.version();
            if requested_version.map(|v| v == current).unwrap_or(true) {
                return Ok(current);
            }
        }
    }

    // 解析目标版本和模型目录
    let (model_dir, target_version) = {
        let config_guard = app_state.config.read().await;
        let captcha = &config_guard.get().captcha;
        let model_path = config_guard.onnx_model_path();
        let target = requested_version.unwrap_or(captcha.model_version);
        (model_path, target)
    };

    // 检查模型文件是否存在
    let missing = OcrBackend::missing_model_files(target_version, &model_dir);
    if !missing.is_empty() {
        return Err(anyhow!(
            "OCR model files incomplete, missing: {}",
            missing.join(", ")
        ));
    }

    // spawn_blocking 中实际加载并替换
    let app_state_for_blocking = app_state.clone();
    let loaded = tokio::task::spawn_blocking(move || -> Result<ModelVersion> {
        let backend = OcrBackend::load(target_version, &model_dir)
            .map_err(|e| anyhow!("OcrBackend::load failed: {}", e))?;
        let actual_version = backend.version();
        let mut guard = app_state_for_blocking
            .local_ocr
            .lock()
            .expect("local_ocr mutex poisoned");
        *guard = Some(backend);
        tracing::info!(
            "[OcrHttpServer] lazy-loaded ONNX OCR model: version={} dir={}",
            actual_version.as_str(),
            model_dir.display()
        );
        Ok(actual_version)
    })
    .await
    .map_err(|e| anyhow!("spawn_blocking join: {}", e))??;

    Ok(loaded)
}

// ============================================================================
// 工具
// ============================================================================

fn json_response(status: StatusCode, body: OcrResponse) -> Response {
    (status, Json(body)).into_response()
}

fn is_authorized(headers: &HeaderMap, expected: &str) -> bool {
    if expected.is_empty() {
        return false;
    }
    let h = match headers.get("authorization").and_then(|v| v.to_str().ok()) {
        Some(s) => s,
        None => return false,
    };
    if h.len() > 7 && h[..7].eq_ignore_ascii_case("Bearer ") {
        h[7..].trim() == expected
    } else {
        false
    }
}

fn generate_token() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
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

// ============================================================================
// 配置同步辅助 (供 commands/ocr_server.rs 调用)
// ============================================================================

/// 从 config 读取 ocr_server 段 (字段定义在 `config::CaptchaConfig` 中)。
pub struct OcrServerConfigSnapshot {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
}

pub async fn snapshot_from_config(
    manager: &OcrHttpServerManager,
    config: &Arc<RwLock<TomlConfig>>,
) -> OcrServerConfigSnapshot {
    let cfg = config.read().await;
    let enabled = cfg.get().captcha.ocr_server_enabled;
    let port = cfg.get().captcha.ocr_server_port;
    drop(cfg);
    OcrServerConfigSnapshot {
        enabled,
        port,
        token: manager.get_token().await,
    }
}
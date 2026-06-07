//! RESTful 远程访问客户端（替代旧的 TCP P2P）
//!
//! 提供对其他设备（手机）的 Web 服务器的 HTTP 调用：
//! - 获取对方信息
//! - 浏览身份/账单
//! - 导出/导入账单

use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// 已配对的远程设备会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSession {
    pub session_id: String,
    pub base_url: String,
    pub device_name: String,
    pub token: String,
}

/// 远程调用错误
#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("网络错误: {0}")]
    Network(String),
    #[error("HTTP {0}")]
    Http(u16, String),
    #[error("解析错误: {0}")]
    Parse(String),
    #[error("无效 URL: {0}")]
    InvalidUrl(String),
}

pub type RemoteResult<T> = Result<T, RemoteError>;

/// 远程访问管理器
pub struct RemoteManager {
    client: Client,
    sessions: Arc<RwLock<HashMap<String, RemoteSession>>>,
}

impl RemoteManager {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 验证 base_url 安全（http/https + host）
    fn validate_base_url(url: &str) -> RemoteResult<()> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(RemoteError::InvalidUrl("scheme must be http/https".into()));
        }
        // 简单校验必须含 host
        let stripped = url
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        if stripped.is_empty() || !stripped.contains('/') && !stripped.contains(':') {
            return Err(RemoteError::InvalidUrl("missing host".into()));
        }
        Ok(())
    }

    /// 连接到远程设备（获取 token）
    pub async fn connect(&self, base_url: String, device_name: String) -> RemoteResult<RemoteSession> {
        Self::validate_base_url(&base_url)?;
        let url = format!("{}/api/auth", base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "device_name": device_name }))
            .send()
            .await
            .map_err(|e| RemoteError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(RemoteError::Http(resp.status().as_u16(), "auth failed".into()));
        }
        #[derive(Deserialize)]
        struct AuthResp {
            success: bool,
            data: Option<AuthData>,
        }
        #[derive(Deserialize)]
        struct AuthData {
            token: String,
        }
        let body: AuthResp = resp.json().await.map_err(|e| RemoteError::Parse(e.to_string()))?;
        let token = body
            .data
            .ok_or_else(|| RemoteError::Parse("missing data".into()))?
            .token;

        // 获取设备信息
        let info_url = format!("{}/api/info", base_url.trim_end_matches('/'));
        let info_resp = self
            .client
            .get(&info_url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| RemoteError::Network(e.to_string()))?;
        let info_body: serde_json::Value = info_resp
            .json()
            .await
            .map_err(|e| RemoteError::Parse(e.to_string()))?;
        let device = info_body["data"]["device_name"]
            .as_str()
            .unwrap_or("Remote")
            .to_string();

        let session = RemoteSession {
            session_id: format!("rs-{}", SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)),
            base_url,
            device_name: device,
            token,
        };
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.session_id.clone(), session.clone());
        Ok(session)
    }

    /// 获取远端身份列表
    pub async fn list_identities(&self, session_id: &str) -> RemoteResult<Vec<serde_json::Value>> {
        let session = self.get_session(session_id).await?;
        let url = format!("{}/api/identities", session.base_url.trim_end_matches('/'));
        self.fetch_json_array(&url, &session.token).await
    }

    /// 获取远端账单列表
    pub async fn list_bills(
        &self,
        session_id: &str,
        query: &HashMap<String, String>,
    ) -> RemoteResult<Vec<serde_json::Value>> {
        let session = self.get_session(session_id).await?;
        let mut url = format!("{}/api/bills", session.base_url.trim_end_matches('/'));
        if !query.is_empty() {
            let qs: Vec<String> = query.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            url.push('?');
            url.push_str(&qs.join("&"));
        }
        // Bills 接口返回 PagedBills 结构（items 字段为数组），需要特殊处理
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(|e| RemoteError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(RemoteError::Http(resp.status().as_u16(), "list bills failed".into()));
        }
        #[derive(Deserialize)]
        struct PagedResp {
            success: bool,
            data: PagedData,
        }
        #[derive(Deserialize)]
        struct PagedData {
            items: String, // 嵌套 JSON 字符串
        }
        let body: PagedResp = resp.json().await.map_err(|e| RemoteError::Parse(e.to_string()))?;
        if !body.success {
            return Err(RemoteError::Parse("server returned failure".into()));
        }
        let items: Vec<serde_json::Value> =
            serde_json::from_str(&body.data.items).map_err(|e| RemoteError::Parse(e.to_string()))?;
        Ok(items)
    }

    /// 导出远端账单为 base64 JSON
    pub async fn export_json(&self, session_id: &str) -> RemoteResult<String> {
        let session = self.get_session(session_id).await?;
        let url = format!("{}/api/export.json", session.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(|e| RemoteError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(RemoteError::Http(resp.status().as_u16(), "export failed".into()));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| RemoteError::Parse(e.to_string()))?;
        if body["success"].as_bool() != Some(true) {
            return Err(RemoteError::Parse("export failure".into()));
        }
        let data = body["data"].to_string();
        let bytes = data.as_bytes();
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    /// 关闭会话
    pub async fn disconnect(&self, session_id: &str) -> RemoteResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }

    /// 列出所有活跃会话
    pub async fn list_sessions(&self) -> Vec<RemoteSession> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    async fn get_session(&self, session_id: &str) -> RemoteResult<RemoteSession> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| RemoteError::Parse(format!("session not found: {}", session_id)))
    }

    async fn fetch_json_array(&self, url: &str, token: &str) -> RemoteResult<Vec<serde_json::Value>> {
        let resp = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| RemoteError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(RemoteError::Http(resp.status().as_u16(), "fetch failed".into()));
        }
        #[derive(Deserialize)]
        struct Wrapped {
            success: bool,
            data: Vec<serde_json::Value>,
        }
        let body: Wrapped = resp.json().await.map_err(|e| RemoteError::Parse(e.to_string()))?;
        if !body.success {
            return Err(RemoteError::Parse("server returned failure".into()));
        }
        Ok(body.data)
    }
}

impl Default for RemoteManager {
    fn default() -> Self {
        Self::new()
    }
}

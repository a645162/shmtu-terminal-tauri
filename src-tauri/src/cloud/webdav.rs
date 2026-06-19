use anyhow::{anyhow, Result};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// WebDAV 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavConfig {
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub backup_root: String,
}

impl Default for WebDavConfig {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            username: String::new(),
            password: String::new(),
            backup_root: "shmtu-backup".to_string(),
        }
    }
}

/// WebDAV 备份 Provider
pub struct WebDavProvider {
    client: reqwest::Client,
    config: Option<WebDavConfig>,
}

impl WebDavProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_default();
        Self { client, config: None }
    }

    pub fn configure(&mut self, config: WebDavConfig) {
        self.config = Some(config);
    }

    fn require_config(&self) -> Result<&WebDavConfig> {
        self.config.as_ref().ok_or_else(|| anyhow!("WebDAV not configured"))
    }

    fn base_url(&self) -> Result<String> {
        Ok(self.require_config()?.server_url.trim_end_matches('/').to_string())
    }

    fn auth_header(&self) -> Result<String> {
        let cfg = self.require_config()?;
        let creds = format!("{}:{}", cfg.username, cfg.password);
        Ok(format!("Basic {}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, creds.as_bytes())))
    }

    pub async fn test_connection(&self) -> Result<bool> {
        let cfg = self.require_config()?;
        let url = format!("{}/{}", self.base_url()?, cfg.backup_root.trim_start_matches('/'));
        let req = reqwest::Request::new(Method::from_bytes(b"PROPFIND")?, url.parse()?);
        let resp = self.client.execute(req).await?;
        Ok(resp.status().is_success() || resp.status().as_u16() == 207)
    }

    pub async fn test_write_read(&self) -> Result<String> {
        let cfg = self.require_config()?;
        let prefix = cfg.backup_root.trim_start_matches('/');
        let test_path = format!("{}/.shmtu-test", prefix);
        let test_content = format!("shmtu-cloud-test-{}", chrono::Utc::now().timestamp_millis());
        self.ensure_dir(prefix).await;
        self.upload(&test_path, test_content.as_bytes()).await?;
        let downloaded = self.download(&test_path).await?;
        let downloaded_str = String::from_utf8(downloaded)?;
        let _ = self.delete(&test_path).await;
        if downloaded_str == test_content {
            Ok("✓ 读写验证成功".to_string())
        } else {
            Err(anyhow!("读回内容不匹配"))
        }
    }

    pub async fn upload(&self, remote_path: &str, data: &[u8]) -> Result<UploadResult> {
        let url = format!("{}/{}", self.base_url()?, remote_path.trim_start_matches('/'));
        let parent = remote_path.rfind('/').map(|i| &remote_path[..i]).unwrap_or("");
        if !parent.is_empty() { self.ensure_dir(parent).await; }
        let resp = self.client.put(&url)
            .header("Authorization", self.auth_header()?)
            .body(data.to_vec())
            .send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Upload failed: HTTP {}", resp.status()));
        }
        Ok(UploadResult {
            remote_path: remote_path.to_string(),
            remote_url: url,
            bytes: data.len() as u64,
            uploaded_at: chrono::Utc::now().timestamp_millis(),
        })
    }

    pub async fn download(&self, remote_path: &str) -> Result<Vec<u8>> {
        let url = format!("{}/{}", self.base_url()?, remote_path.trim_start_matches('/'));
        let resp = self.client.get(&url)
            .header("Authorization", self.auth_header()?)
            .send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Download failed: HTTP {}", resp.status()));
        }
        Ok(resp.bytes().await?.to_vec())
    }

    pub async fn list(&self, prefix: &str) -> Result<Vec<BackupMeta>> {
        let url = format!("{}/{}", self.base_url()?, prefix.trim_start_matches('/'));
        let body = r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:getlastmodified/><d:getcontentlength/><d:resourcetype/></d:prop></d:propfind>"#;
        let resp = self.client.request(Method::from_bytes(b"PROPFIND")?, &url)
            .header("Authorization", self.auth_header()?)
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(body.to_string())
            .send().await?;
        if !resp.status().is_success() && resp.status().as_u16() != 207 {
            return Err(anyhow!("List failed: HTTP {}", resp.status()));
        }
        let xml = resp.text().await?;
        Ok(parse_propfind(&xml))
    }

    pub async fn delete(&self, remote_path: &str) -> Result<bool> {
        let url = format!("{}/{}", self.base_url()?, remote_path.trim_start_matches('/'));
        let resp = self.client.delete(&url)
            .header("Authorization", self.auth_header()?)
            .send().await?;
        Ok(resp.status().is_success())
    }

    async fn ensure_dir(&self, dir_path: &str) {
        let url = format!("{}/{}", self.base_url().unwrap_or_default(), dir_path.trim_start_matches('/'));
        let Ok(url_parsed) = url.parse::<reqwest::Url>() else { return };
        let req = reqwest::Request::new(Method::from_bytes(b"MKCOL").unwrap(), url_parsed);
        let _ = self.client.execute(req).await;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    pub remote_path: String,
    pub remote_url: String,
    pub bytes: u64,
    pub uploaded_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMeta {
    pub remote_path: String,
    pub name: String,
    pub size: u64,
    pub last_modified: i64,
}

fn parse_propfind(xml: &str) -> Vec<BackupMeta> {
    let mut results = Vec::new();
    let response_re = regex::Regex::new(r"(?is)<(?:D:)?response[^>]*>(.*?)</(?:D:)?response>").unwrap();
    let href_re = regex::Regex::new(r"(?i)<(?:D:)?href[^>]*>(.*?)</(?:D:)?href>").unwrap();
    let size_re = regex::Regex::new(r"(?i)<(?:D:)?getcontentlength[^>]*>(\d+)</(?:D:)?getcontentlength>").unwrap();
    let is_collection_re = regex::Regex::new(r"(?is)<(?:D:)?resourcetype[^>]*>.*<(?:D:)?collection.*</(?:D:)?resourcetype>").unwrap();

    for cap in response_re.captures_iter(xml) {
        let body = cap.get(1).unwrap().as_str();
        if is_collection_re.is_match(body) { continue; }
        let href = match href_re.captures(body) {
            Some(c) => c.get(1).unwrap().as_str().to_string(),
            None => continue,
        };
        let name = href.rsplit('/').next().unwrap_or("").to_string();
        if name.is_empty() { continue; }
        let size: u64 = size_re.captures(body).and_then(|c| c.get(1)?.as_str().parse().ok()).unwrap_or(0);
        results.push(BackupMeta { remote_path: href.trim_start_matches('/').to_string(), name, size, last_modified: 0 });
    }
    results
}

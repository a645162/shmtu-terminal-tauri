//! 数据库文件管理器 — 支持本地加载 + GitHub 云端下载
//!
//! 数据库文件位于 `database/bill/` 目录：
//! - rules.toml    — 统一分类规则（类型+位置+时段）
//! - position.json — 对方账户→位置翻译表
//! - type.json     — 账单类型识别规则
//! - schedule.json — 食堂营业时间表
//!
//! 加载策略：本地文件存在则直接加载，不存在则从 GitHub 下载后加载。

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use shmtu_cas::classifier::PositionTranslator;

/// GitHub raw 文件基础 URL
const GITHUB_RAW_BASE: &str =
    "https://raw.githubusercontent.com/a645162/shmtu-terminal/main/database/bill";

/// 数据库文件名列表
const DB_FILES: &[&str] = &[
    "rules.toml",
    "position.toml",
    "type.toml",
    "schedule.toml",
];

/// 数据库文件管理器
pub struct DatabaseFileManager {
    /// 本地数据库目录路径
    local_dir: PathBuf,
}

impl DatabaseFileManager {
    /// 创建管理器，指定本地数据库目录
    pub fn new(local_dir: impl AsRef<Path>) -> Self {
        Self {
            local_dir: local_dir.as_ref().to_path_buf(),
        }
    }

    /// 确保本地数据库目录存在，如果缺失则从 GitHub 下载
    pub async fn ensure_local_files(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.local_dir)
            .map_err(|e| AppError::Config(format!("创建数据库目录失败: {}", e)))?;

        for filename in DB_FILES {
            let local_path = self.local_dir.join(filename);
            if !local_path.exists() {
                tracing::info!(
                    "[DB] 本地文件缺失: {:?}，从 GitHub 下载...",
                    filename
                );
                if let Err(e) = self.download_file(filename).await {
                    tracing::warn!("[DB] 下载 {} 失败: {}，将跳过", filename, e);
                }
            }
        }
        Ok(())
    }

    /// 从 GitHub 下载单个文件
    pub async fn download_file(&self, filename: &str) -> AppResult<()> {
        let url = format!("{}/{}", GITHUB_RAW_BASE, filename);
        let local_path = self.local_dir.join(filename);

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Config(format!("下载 {} 失败: {}", filename, e)))?;

        if !response.status().is_success() {
            return Err(AppError::Config(format!(
                "下载 {} 失败，HTTP {}",
                filename,
                response.status()
            )));
        }

        let content = response
            .text()
            .await
            .map_err(|e| AppError::Config(format!("读取 {} 响应失败: {}", filename, e)))?;

        // 备份旧文件
        if local_path.exists() {
            let backup = format!("{}.bak", local_path.to_string_lossy());
            let _ = std::fs::copy(&local_path, &backup);
        }

        std::fs::write(&local_path, &content)
            .map_err(|e| AppError::Config(format!("写入 {} 失败: {}", filename, e)))?;

        tracing::info!("[DB] {} 下载成功 -> {:?}", filename, local_path);
        Ok(())
    }

    /// 下载所有文件（强制更新）
    pub async fn download_all(&self) -> AppResult<()> {
        for filename in DB_FILES {
            self.download_file(filename).await?;
        }
        Ok(())
    }

    /// 读取本地文件内容
    pub fn read_file(&self, filename: &str) -> AppResult<String> {
        let path = self.local_dir.join(filename);
        std::fs::read_to_string(&path)
            .map_err(|e| AppError::Config(format!("读取 {:?} 失败: {}", path, e)))
    }

    /// 获取 rules.toml 路径
    pub fn rules_path(&self) -> PathBuf {
        self.local_dir.join("rules.toml")
    }

    /// 获取 position.toml 内容
    pub fn load_position_toml(&self) -> AppResult<String> {
        self.read_file("position.toml")
    }

    /// 获取 type.toml 内容
    pub fn load_type_toml(&self) -> AppResult<String> {
        self.read_file("type.toml")
    }

    /// 获取 schedule.toml 内容
    pub fn load_schedule_toml(&self) -> AppResult<String> {
        self.read_file("schedule.toml")
    }

    /// 从本地 position.toml 创建位置翻译器（使用 shmtu-cas 的自动格式识别）
    pub fn create_position_translator(&self) -> PositionTranslator {
        let path = self.local_dir.join("position.toml");
        PositionTranslator::from_file(&path).unwrap_or_default()
    }
}

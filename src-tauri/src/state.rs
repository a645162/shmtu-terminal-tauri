use std::sync::Arc;

use tokio::sync::RwLock;

use crate::classification::BillClassifier;
use crate::config::TomlConfig;
use crate::crypto::CryptoService;
use crate::database::DatabaseFileManager;
use crate::db::DatabaseManager;
use crate::error::AppResult;
use crate::export::ExportService;
use crate::sync::BillSyncService;

/// 应用全局状态，通过 tauri::State 注入到所有命令中
pub struct AppState {
    pub db_manager: Arc<RwLock<DatabaseManager>>,
    pub crypto: Arc<RwLock<CryptoService>>,
    pub config: Arc<RwLock<TomlConfig>>,
    pub sync_service: Arc<RwLock<BillSyncService>>,
    pub export_service: Arc<RwLock<ExportService>>,
    pub classifier: Arc<RwLock<Option<BillClassifier>>>,
    /// 数据库文件管理器（支持本地+GitHub云端加载）
    pub db_file_manager: Arc<DatabaseFileManager>,
}

impl AppState {
    /// 初始化应用状态
    pub async fn init(data_dir: &str) -> AppResult<Self> {
        let db_manager = DatabaseManager::connect(data_dir).await?;

        let crypto = CryptoService::from_device_id("shmtu-terminal-device-key");

        let config = TomlConfig::load(data_dir)?;

        // 数据库文件管理器 — 本地目录，不存在时从 GitHub 下载
        let db_local_dir = std::path::Path::new(data_dir).join("database").join("bill");
        let db_file_manager = DatabaseFileManager::new(&db_local_dir);

        // 确保本地数据库文件存在（缺失则从 GitHub 下载）
        if let Err(e) = db_file_manager.ensure_local_files().await {
            tracing::warn!("数据库文件初始化失败（将使用空分类器）: {}", e);
        }

        // 从本地文件创建位置翻译器
        let position_translator = db_file_manager.create_position_translator();

        let sync_service = BillSyncService::new(
            db_manager.clone_ref(),
            CryptoService::from_device_id("shmtu-terminal-device-key"),
            position_translator.clone(),
        );

        let export_service = ExportService::new(db_manager.clone_ref(), position_translator);

        // 从本地文件加载分类器
        let classifier = {
            let rules_path = db_file_manager.rules_path();
            if rules_path.exists() {
                match BillClassifier::from_file(rules_path.to_str().unwrap_or("")) {
                    Ok(c) => {
                        tracing::info!("分类器加载成功: {:?}", rules_path);
                        Some(c)
                    }
                    Err(e) => {
                        tracing::warn!("分类器加载失败: {}", e);
                        None
                    }
                }
            } else {
                tracing::warn!("规则文件不存在: {:?}，分类功能不可用", rules_path);
                None
            }
        };

        Ok(Self {
            db_manager: Arc::new(RwLock::new(db_manager)),
            crypto: Arc::new(RwLock::new(crypto)),
            config: Arc::new(RwLock::new(config)),
            sync_service: Arc::new(RwLock::new(sync_service)),
            export_service: Arc::new(RwLock::new(export_service)),
            classifier: Arc::new(RwLock::new(classifier)),
            db_file_manager: Arc::new(db_file_manager),
        })
    }

    /// 重新加载分类器（规则文件变更后调用）
    pub async fn reload_classifier(&self) -> AppResult<()> {
        let rules_path = self.db_file_manager.rules_path();
        let new_classifier = if rules_path.exists() {
            Some(BillClassifier::from_file(
                rules_path.to_str().unwrap_or(""),
            )?)
        } else {
            None
        };
        let mut classifier = self.classifier.write().await;
        *classifier = new_classifier;
        Ok(())
    }

    /// 从 GitHub 更新所有数据库文件并重新加载分类器
    pub async fn update_database_from_remote(&self) -> AppResult<()> {
        self.db_file_manager.download_all().await?;
        self.reload_classifier().await
    }
}

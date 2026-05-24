use std::sync::Arc;

use tokio::sync::RwLock;

use crate::classification::BillClassifier;
use crate::config::TomlConfig;
use crate::crypto::CryptoService;
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
}

impl AppState {
    /// 初始化应用状态
    pub async fn init(data_dir: &str) -> AppResult<Self> {
        let db_manager = DatabaseManager::connect(data_dir).await?;

        let crypto = CryptoService::from_device_id("shmtu-terminal-device-key");

        let config = TomlConfig::load(data_dir)?;

        let sync_service = BillSyncService::new(
            db_manager.clone_ref(),
            CryptoService::from_device_id("shmtu-terminal-device-key"),
        );

        let export_service = ExportService::new(db_manager.clone_ref());

        // 尝试加载分类器
        let classifier = {
            let rules_path = config.classification_rules_path();
            if rules_path.exists() {
                BillClassifier::from_file(rules_path.to_str().unwrap_or("")).ok()
            } else {
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
        })
    }

    /// 重新加载分类器（规则文件变更后调用）
    pub async fn reload_classifier(&self) -> AppResult<()> {
        let config = self.config.read().await;
        let rules_path = config.classification_rules_path();
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
}

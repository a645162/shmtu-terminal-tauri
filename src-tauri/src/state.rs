use std::sync::{atomic::AtomicBool, Arc};

use shmtu_cas::cas::epay::EpayAuth;
use shmtu_ocr::backend::CasOnnxBackend;
use shmtu_p2p::P2PManager;
use tokio::sync::{Mutex, RwLock};

use crate::auto_sync::AutoSyncService;
use crate::classification::BillClassifier;
use crate::config::TomlConfig;
use crate::crypto::CryptoService;
use crate::database::DatabaseFileManager;
use crate::db::DatabaseManager;
use crate::error::AppResult;
use crate::export::ExportService;
use crate::session_refresh::SessionExpirationService;
use crate::sync::BillSyncService;

pub struct CaptchaTestSession {
    pub epay: EpayAuth,
    pub execution: String,
}

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
    /// Session 过期检查服务
    pub session_expiration_service: Arc<SessionExpirationService>,
    /// 账单自动同步服务
    pub auto_sync_service: Arc<AutoSyncService>,
    /// 本地 ONNX 推理后端（CPU 密集同步操作，使用 std::sync::Mutex）
    pub local_ocr: Arc<std::sync::Mutex<Option<CasOnnxBackend>>>,
    /// 本地 ONNX 模型下载取消标记
    pub local_ocr_download_cancel: Arc<AtomicBool>,
    /// 本地 ONNX 模型下载运行标记
    pub local_ocr_download_active: Arc<AtomicBool>,
    /// 串行化本地 ONNX 模型下载任务
    pub local_ocr_download_lock: Arc<Mutex<()>>,
    /// 验证码测试使用的待提交 challenge，会话需与展示的验证码保持一致。
    pub captcha_test_session: Arc<Mutex<Option<CaptchaTestSession>>>,
    /// P2P 数据传输管理器
    pub p2p_manager: Arc<RwLock<P2PManager>>,
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
        let sync_service_arc = Arc::new(RwLock::new(sync_service));

        // Session 过期检查服务 - 创建 Arc 包装
        let config_arc: Arc<RwLock<TomlConfig>> = Arc::new(RwLock::new(config));
        let db_manager_arc = Arc::new(db_manager.clone_ref());
        let session_expiration_service = Arc::new(SessionExpirationService::new(
            config_arc.clone(),
            db_manager_arc,
            Arc::new(CryptoService::from_device_id("shmtu-terminal-device-key")),
        ));
        let auto_sync_service = Arc::new(AutoSyncService::new(
            config_arc.clone(),
            sync_service_arc.clone(),
        ));
        // 异步启动 session 过期检查服务
        let expiration_service = session_expiration_service.clone();
        tokio::spawn(async move {
            expiration_service.start().await;
        });
        let auto_sync = auto_sync_service.clone();
        tokio::spawn(async move {
            auto_sync.start().await;
        });

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
            config: config_arc,
            sync_service: sync_service_arc,
            export_service: Arc::new(RwLock::new(export_service)),
            classifier: Arc::new(RwLock::new(classifier)),
            db_file_manager: Arc::new(db_file_manager),
            session_expiration_service,
            auto_sync_service,
            local_ocr: Arc::new(std::sync::Mutex::new(None)),
            local_ocr_download_cancel: Arc::new(AtomicBool::new(false)),
            local_ocr_download_active: Arc::new(AtomicBool::new(false)),
            local_ocr_download_lock: Arc::new(Mutex::new(())),
            captcha_test_session: Arc::new(Mutex::new(None)),
            p2p_manager: Arc::new(RwLock::new(P2PManager::new())),
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

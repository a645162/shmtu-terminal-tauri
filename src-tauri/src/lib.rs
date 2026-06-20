pub mod auto_sync;
pub mod classification;
pub mod cloud;
pub mod commands;
pub mod ocr_server;
pub mod config;
pub mod crypto;
pub mod database;
pub mod db;
pub mod entity;
pub mod error;
pub mod export;
pub mod models;
pub mod p2p;
pub mod remote;
pub mod session_refresh;
pub mod state;
pub mod sync;

use std::sync::Arc;

use commands::{
    account, bill, captcha, classify, cloud as cmd_cloud, config as cmd_config, data, debug as cmd_debug,
    error as error_cmd, identity, ocr_server as cmd_ocr_server, p2p as cmd_p2p,
    person_account as cmd_person_account, remote as cmd_remote, statistics, sync as cmd_sync,
};
use tauri::Manager;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,sea_orm_migration=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("海大终端启动中...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data dir");
            tracing::info!("数据目录: {:?}", data_dir);

            let legacy_data = std::path::Path::new("Data");
            if !data_dir.exists() && legacy_data.exists() {
                if let Err(e) = std::fs::create_dir_all(&data_dir) {
                    tracing::error!("创建数据目录失败: {}", e);
                } else {
                    if let Ok(entries) = std::fs::read_dir(legacy_data) {
                        for entry in entries.flatten() {
                            let src = entry.path();
                            let dst = data_dir.join(entry.file_name());
                            if let Err(e) = std::fs::rename(&src, &dst) {
                                tracing::error!("迁移数据失败 {:?}: {}", src, e);
                            }
                        }
                    }
                }
            }

            let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            let app_state = runtime
                .block_on(state::AppState::init(data_dir.to_str().unwrap_or("Data")))
                .expect("Failed to initialize app state");

            // P2P auto-start removed: now uses RESTful WebServer

            tracing::info!("应用状态初始化完成");
            app.manage(app_state);
            app.manage(remote::RemoteManager::new());

            // OCR HTTP 服务器管理器 (懒加载模型,首次 POST /api/ocr 才加载)
            let ocr_http = ocr_server::OcrHttpServerManager::new(
                std::env::var("HOSTNAME").unwrap_or_else(|_| "Tauri".to_string())
            );
            // 若配置启用且端口可用,则自动启动 (setup 是同步上下文,用 block_on)
            {
                let app_state_clone: state::AppState = app.state::<state::AppState>().inner().clone();
                let manager_for_check = ocr_http.clone();
                let cfg_snapshot = {
                    let cfg_guard = tauri::async_runtime::block_on(app_state_clone.config.read());
                    cfg_guard.get().captcha.clone()
                };
                if cfg_snapshot.ocr_server_enabled {
                    let port_to_use = cfg_snapshot.ocr_server_port;
                    let bind_ip = config::ocr_server_bind_address(
                        &cfg_snapshot.ocr_server_scope,
                        &cfg_snapshot.ocr_server_bind_addr,
                    );
                    if let Err(e) = tauri::async_runtime::block_on(
                        manager_for_check.start(
                            port_to_use,
                            bind_ip,
                            Arc::new(app_state_clone),
                        )
                    ) {
                        tracing::warn!("[OcrHttpServer] auto-start failed: {}", e);
                    } else {
                        tracing::info!(
                            "[OcrHttpServer] auto-started on {}:{} (scope={:?})",
                            bind_ip, port_to_use, cfg_snapshot.ocr_server_scope
                        );
                    }
                }
            }
            app.manage(ocr_http);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            identity::list_identities,
            identity::create_identity,
            identity::update_identity,
            identity::delete_identity,
            identity::set_default_identity,
            identity::get_default_identity,
            identity::set_last_identity,
            identity::get_last_identity,
            account::list_accounts,
            account::create_account,
            account::update_account,
            account::delete_account,
            bill::query_bills,
            bill::get_bill_detail,
            bill::delete_merged_bill,
            bill::update_bill_notes,
            bill::dedupe_identity_bills,
            bill::dedupe_account_bills,
            bill::rebuild_merged_bills,
            cmd_sync::incremental_sync,
            cmd_sync::full_sync,
            cmd_sync::incremental_sync_account,
            cmd_sync::full_sync_account,
            cmd_sync::get_sync_progress,
            cmd_sync::cas_login,
            cmd_sync::check_login_status,
            cmd_sync::sync_with_captcha,
            cmd_sync::refresh_captcha,
            captcha::get_captcha_image,
            captcha::get_captcha_with_execution,
            captcha::get_local_ocr_model_status,
            captcha::ensure_local_ocr_models,
            captcha::cancel_local_ocr_model_download,
            captcha::delete_local_ocr_models,
            captcha::test_captcha,
            captcha::batch_test_captcha,
            captcha::init_local_ocr,
            captcha::unload_local_ocr,
            captcha::get_ocr_model_version,
            captcha::set_ocr_model_version,
            captcha::get_ocr_v2_tag_catalog,
            captcha::refresh_ocr_v2_tag_catalog,
            captcha::get_ocr_v2_model_tag,
            captcha::set_ocr_v2_model_tag,
            captcha::ocr_v2_resolve_latest_tag,
            captcha::list_ocr_v2_models,
            captcha::set_ocr_v2_backbone,
            captcha::set_ocr_v2_precision,
            captcha::get_ocr_v2_config,
            captcha::scan_local_ocr_models,
            captcha::select_local_ocr_model,
            data::export_data,
            data::import_data,
            data::list_snapshots,
            data::create_snapshot,
            data::restore_snapshot,
            cmd_config::load_config,
            cmd_config::save_config,
            cmd_config::verify_startup_password,
            cmd_config::set_startup_password,
            cmd_config::get_app_version,
            cmd_config::check_for_updates,
            cmd_config::get_git_contributors,
            cmd_config::get_auto_sync_status,
            cmd_config::get_session_expiration_status,
            cmd_config::check_session_expiration,
            cmd_config::restart_session_expiration_service,
            statistics::get_statistics_summary,
            statistics::get_daily_trend,
            statistics::get_category_distribution,
            statistics::get_meal_distribution,
            statistics::get_consumption_distribution,
            statistics::get_merchant_ranking,
            statistics::get_category_summary,
            statistics::get_forgot_card_stats,
            statistics::get_category_bills,
            classify::translate_target,
            classify::classify_bill,
            classify::get_bill_statistics,
            classify::get_classification_rules,
            classify::reclassify_all_bills,
            classify::reclassify_bills_by_identity,
            error_cmd::log_error,
            cmd_remote::remote_connect,
            cmd_remote::remote_disconnect,
            cmd_remote::remote_list_sessions,
            cmd_remote::remote_list_identities,
            cmd_remote::remote_list_bills,
            cmd_remote::remote_export,
            cmd_person_account::fetch_person_account,
            cmd_person_account::get_cached_person_account,
            cmd_person_account::list_cached_person_accounts,
            cmd_person_account::submit_person_account_captcha,
            cmd_debug::clear_all_cookies,
            cmd_debug::clear_all_cookies,
            // 云备份
            cmd_cloud::cloud_backup_get_config,
            cmd_cloud::cloud_backup_save_config,
            cmd_cloud::cloud_backup_test_connection,
            cmd_cloud::cloud_backup_test_write_read,
            cmd_cloud::cloud_backup_now,
            cmd_cloud::cloud_backup_restore,
            cmd_cloud::cloud_backup_list_remote,
            cmd_cloud::cloud_backup_delete_remote,
            cmd_cloud::cloud_backup_get_auto_config,
            cmd_cloud::cloud_backup_set_auto_enabled,
            cmd_cloud::cloud_backup_set_auto_interval,
            cmd_cloud::cloud_backup_set_max_keep,
            // P2P RESTful（与 Android P2PHttpServer 协议对齐）
            cmd_p2p::p2p_get_status,
            cmd_p2p::p2p_start_server,
            cmd_p2p::p2p_stop_server,
            cmd_p2p::p2p_set_pair_code,
            cmd_p2p::p2p_discover,
            cmd_p2p::p2p_pair,
            cmd_p2p::p2p_upload_transfer,
            cmd_p2p::p2p_download_transfer,
            // OCR HTTP 服务器 (懒加载,与 Android OcrWebServer 协议对齐)
            cmd_ocr_server::ocr_server_start,
            cmd_ocr_server::ocr_server_stop,
            cmd_ocr_server::ocr_server_status,
            cmd_ocr_server::ocr_server_rotate_token,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

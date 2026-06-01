pub mod classification;
pub mod commands;
pub mod config;
pub mod crypto;
pub mod database;
pub mod db;
pub mod entity;
pub mod error;
pub mod export;
pub mod models;
pub mod session_refresh;
pub mod state;
pub mod sync;

use commands::{
    account, bill, captcha, classify, config as cmd_config, data, error as error_cmd, identity, statistics, sync as cmd_sync,
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

            tracing::info!("应用状态初始化完成");
            app.manage(app_state);
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
            captcha::test_captcha,
            captcha::batch_test_captcha,
            captcha::init_local_ocr,
            captcha::unload_local_ocr,
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
            error_cmd::log_error,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

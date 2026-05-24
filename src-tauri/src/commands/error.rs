use tauri::State;
use crate::state::AppState;
use std::sync::atomic::{AtomicU64, Ordering};

static ERROR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tauri::command]
pub async fn log_error(
    state: State<'_, AppState>,
    message: String,
) -> Result<(), String> {
    let count = ERROR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let error_entry = format!(
        "[{}] #{} Error: {}\n",
        timestamp, count, message
    );

    // 记录到日志
    tracing::error!("[Frontend Error] #{}: {}", count, message);

    // 保存到错误日志文件
    let data_dir = {
        let db = state.db_manager.read().await;
        db.data_dir().to_path_buf()
    };
    let error_log_path = data_dir.join("frontend_errors.log");

    if let Err(e) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&error_log_path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(error_entry.as_bytes())
        })
    {
        tracing::warn!("Failed to write error log: {}", e);
    }

    Ok(())
}
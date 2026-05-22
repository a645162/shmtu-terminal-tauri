use serde::{Deserialize, Serialize};
use tauri::State;

use crate::export::{ExportFormat, ExportOptions, SnapshotInfo};
use crate::state::AppState;

/// 前端导出参数（与 tauri.ts ExportParams 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportParamsFrontend {
    pub identity_id: i64,
    pub format: String,
    pub source_type: String,
    pub file_path: String,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
}

/// 前端快照信息（与 tauri.ts SnapshotInfo 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfoFrontend {
    pub filename: String,
    pub created_at: String,
    pub size_bytes: u64,
}

impl From<SnapshotInfo> for SnapshotInfoFrontend {
    fn from(s: SnapshotInfo) -> Self {
        Self {
            filename: s.name,
            created_at: s
                .created_at
                .map(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default()
                })
                .unwrap_or_default(),
            size_bytes: s.size,
        }
    }
}

#[tauri::command]
pub async fn export_data(
    state: State<'_, AppState>,
    params: ExportParamsFrontend,
) -> Result<String, String> {
    let export_format = match params.format.as_str() {
        "csv" => ExportFormat::Csv,
        "json" => ExportFormat::Json,
        "qianji" => ExportFormat::Qianji,
        _ => return Err(format!("不支持的导出格式: {}", params.format)),
    };

    let start_timestamp = params.date_start.as_ref().and_then(|d| {
        chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%d").ok()
            .map(|dt| dt.and_utc().timestamp())
    });
    let end_timestamp = params.date_end.as_ref().and_then(|d| {
        chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%d").ok()
            .map(|dt| dt.and_utc().timestamp())
    });

    let options = ExportOptions {
        format: export_format,
        output_path: params.file_path.clone(),
        include_classification: true,
        start_timestamp,
        end_timestamp,
    };

    let export_service = state.export_service.read().await;

    if params.source_type == "original" {
        // 导出原始数据需要找到账号ID
        let db = state.db_manager.read().await;
        let accounts = db.list_accounts_by_identity(params.identity_id).map_err(|e| e.to_string())?;
        if let Some(account) = accounts.first() {
            export_service
                .export_account_bills(&account.account_id, &options)
                .map_err(|e| e.to_string())?;
        }
    } else {
        // 获取身份名称
        let db = state.db_manager.read().await;
        let identity = db.get_identity(params.identity_id).map_err(|e| e.to_string())?;
        let identity_name = identity
            .as_ref()
            .map(|i| i.name.as_str())
            .unwrap_or("unknown");

        export_service
            .export_identity_bills(params.identity_id, identity_name, &options)
            .map_err(|e| e.to_string())?;
    }

    Ok(params.file_path)
}

#[tauri::command]
pub async fn import_data(
    state: State<'_, AppState>,
    file_path: String,
    identity_id: i64,
) -> Result<usize, String> {
    let export_service = state.export_service.read().await;
    export_service
        .import_json(identity_id, &file_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_snapshots(state: State<'_, AppState>) -> Result<Vec<SnapshotInfoFrontend>, String> {
    let export_service = state.export_service.read().await;
    let snapshots = export_service.list_snapshots().map_err(|e| e.to_string())?;
    Ok(snapshots.into_iter().map(SnapshotInfoFrontend::from).collect())
}

#[tauri::command]
pub async fn create_snapshot(state: State<'_, AppState>) -> Result<SnapshotInfoFrontend, String> {
    let config = state.config.read().await;
    let max_keep = config.get().data.snapshot_keep_count;
    drop(config);

    let export_service = state.export_service.read().await;
    let path = export_service
        .create_snapshot(max_keep)
        .map_err(|e| e.to_string())?;

    // 获取创建的快照信息
    let snapshots = export_service.list_snapshots().map_err(|e| e.to_string())?;
    let snapshot = snapshots
        .into_iter()
        .find(|s| s.path == path)
        .ok_or_else(|| "快照创建成功但未找到".to_string())?;

    Ok(SnapshotInfoFrontend::from(snapshot))
}

#[tauri::command]
pub async fn restore_snapshot(state: State<'_, AppState>, filename: String) -> Result<(), String> {
    let export_service = state.export_service.read().await;
    let snapshot_dir = state.db_manager.read().await.snapshot_dir();
    let snapshot_path = snapshot_dir.join(&filename);

    if !snapshot_path.exists() {
        return Err(format!("快照文件不存在: {}", filename));
    }

    export_service
        .restore_snapshot(snapshot_path.to_str().unwrap_or(""))
        .map_err(|e| e.to_string())
}

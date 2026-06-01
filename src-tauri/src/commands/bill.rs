use serde::{Deserialize, Serialize};
use sea_orm::{ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use tauri::State;

use crate::db::{init::bill_merged_model_to_app, BillStoreImpl};
use crate::entity::bill_merged;
use crate::models::BillMerged;
use crate::state::AppState;

fn normalize_bill_type_status(bill_type: &str) -> Option<&'static str> {
    match bill_type {
        "success" => Some("交易成功"),
        "not_paid" => Some("#waitfor"),
        "failure" => Some("#fail"),
        _ => None,
    }
}

/// 前端账单查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillQueryParams {
    pub identity_id: Option<i64>,
    pub account_id: Option<String>,
    pub bill_type: String,
    pub page: u32,
    pub page_size: u32,
    pub keyword: Option<String>,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
}

/// 账单查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillQueryResult {
    pub items: Vec<BillItemFrontend>,
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
}

/// 去重操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeResult {
    pub backfilled_count: usize,
    pub removed_count: usize,
}

/// 前端展示的账单项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillItemFrontend {
    pub id: i64,
    pub date_str: String,
    pub time_str: String,
    pub time_str_formatted: Option<String>,
    pub date_time_formatted: Option<String>,
    pub end_date_time_formatted: Option<String>,
    pub timestamp: Option<i64>,
    pub end_timestamp: Option<i64>,
    pub item_type: Option<String>,
    pub number: Option<String>,
    pub number_list: Option<String>,
    pub target_user: Option<String>,
    pub money_str: Option<String>,
    pub money: Option<f64>,
    pub method: Option<String>,
    pub status_str: Option<String>,
    pub is_combined: bool,
    pub account_id: Option<String>,
    pub synced_at: Option<String>,
    pub source_account_id: Option<String>,
    pub is_manual: Option<bool>,
    pub position: Option<String>,
    pub room: Option<String>,
    pub notes: Option<String>,
}

impl From<BillMerged> for BillItemFrontend {
    fn from(b: BillMerged) -> Self {
        Self {
            id: b.id,
            date_str: b.date_str,
            time_str: b.time_str,
            time_str_formatted: b.time_str_formatted,
            date_time_formatted: b.date_time_formatted,
            end_date_time_formatted: b.end_date_time_formatted,
            timestamp: b.timestamp,
            end_timestamp: b.end_timestamp,
            item_type: b.item_type,
            number: b.number,
            number_list: b.number_list,
            target_user: b.target_user,
            money_str: b.money_str,
            money: b.money,
            method: b.method,
            status_str: b.status_str,
            is_combined: b.is_combined,
            account_id: None,
            synced_at: b.synced_at,
            source_account_id: b.source_account_id,
            is_manual: Some(b.is_manual),
            position: b.position,
            room: b.room,
            notes: b.notes,
        }
    }
}

/// 分页查询合并账单。
///
/// 先尝试回填缺失的元数据，再按页码返回结果。
#[tauri::command]
pub async fn query_bills(
    state: State<'_, AppState>,
    params: BillQueryParams,
) -> Result<BillQueryResult, String> {
    tracing::debug!(
        "[Bill] query_bills: identity_id={:?}, page={}, page_size={}",
        params.identity_id,
        params.page,
        params.page_size
    );

    let db = state.db_manager.read().await;

    let identity_id = params.identity_id.unwrap_or(0);
    if identity_id == 0 {
        return Ok(BillQueryResult {
            items: Vec::new(),
            total: 0,
            page: params.page,
            page_size: params.page_size,
        });
    }

    let db_conn = db.db().clone();
    let translator = state.db_file_manager.create_position_translator();
    let store = BillStoreImpl::new(db_conn, "", identity_id, translator)
        .await
        .map_err(|e| e.to_string())?;

    if let Err(e) = store.backfill_merged_metadata(identity_id).await {
        tracing::warn!(
            "[Bill] backfill_merged_metadata failed for identity {}: {}",
            identity_id,
            e
        );
    }

    let _ = store
        .list_merged_bills(identity_id, params.page, params.page_size)
        .await
        .map_err(|e| e.to_string())?;

    let mut condition = Condition::all().add(bill_merged::Column::IdentityId.eq(identity_id));

    if let Some(account_id) = params.account_id.clone() {
        condition = condition.add(bill_merged::Column::SourceAccountId.eq(account_id));
    }

    if let Some(status) = normalize_bill_type_status(&params.bill_type) {
        condition = condition.add(bill_merged::Column::StatusStr.eq(status));
    }

    if let Some(date_start) = params.date_start.clone() {
        condition = condition.add(bill_merged::Column::DateStr.gte(date_start.replace('-', ".")));
    }

    if let Some(date_end) = params.date_end.clone() {
        condition = condition.add(bill_merged::Column::DateStr.lte(date_end.replace('-', ".")));
    }

    if let Some(keyword) = params.keyword.clone().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        condition = condition.add(
            Condition::any()
                .add(bill_merged::Column::ItemType.contains(&keyword))
                .add(bill_merged::Column::TargetUser.contains(&keyword))
                .add(bill_merged::Column::Number.contains(&keyword))
                .add(bill_merged::Column::NumberList.contains(&keyword))
                .add(bill_merged::Column::Position.contains(&keyword))
                .add(bill_merged::Column::Room.contains(&keyword))
                .add(bill_merged::Column::Notes.contains(&keyword)),
        );
    }

    let page = params.page.max(1);
    let page_size = params.page_size.max(1);

    let paginator = bill_merged::Entity::find()
        .filter(condition)
        .order_by_desc(bill_merged::Column::Timestamp)
        .order_by_desc(bill_merged::Column::Id)
        .paginate(db.db(), page_size as u64);

    let total = paginator.num_items().await.map_err(|e| e.to_string())? as u32;
    let models = paginator
        .fetch_page((page - 1) as u64)
        .await
        .map_err(|e| e.to_string())?;

    let items: Vec<BillItemFrontend> = models
        .into_iter()
        .map(bill_merged_model_to_app)
        .map(BillItemFrontend::from)
        .collect();

    tracing::debug!(
        "[Bill] query_bills: returned {} items, total={}, page={}, page_size={}, bill_type={}, keyword={:?}, date_start={:?}, date_end={:?}",
        items.len(),
        total,
        page,
        page_size,
        params.bill_type,
        params.keyword,
        params.date_start,
        params.date_end
    );

    Ok(BillQueryResult {
        items,
        total,
        page,
        page_size,
    })
}

/// 根据账单 ID 获取完整账单详情。
#[tauri::command]
pub async fn get_bill_detail(
    state: State<'_, AppState>,
    identity_id: i64,
    bill_id: i64,
) -> Result<BillItemFrontend, String> {
    tracing::debug!(
        "[Bill] get_bill_detail: identity_id={}, bill_id={}",
        identity_id,
        bill_id
    );

    let db = state.db_manager.read().await;

    let model = bill_merged::Entity::find_by_id(bill_id)
        .filter(bill_merged::Column::IdentityId.eq(identity_id))
        .one(db.db())
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Bill not found: {}", bill_id))?;

    Ok(BillItemFrontend::from(bill_merged_model_to_app(model)))
}

/// 删除指定的合并账单。
#[tauri::command]
pub async fn delete_merged_bill(
    state: State<'_, AppState>,
    identity_id: i64,
    bill_id: i64,
) -> Result<(), String> {
    tracing::info!(
        "[Bill] delete_merged_bill: identity_id={}, bill_id={}",
        identity_id,
        bill_id
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db().clone();
    let translator = state.db_file_manager.create_position_translator();
    let store = BillStoreImpl::new(db_conn, "", identity_id, translator)
        .await
        .map_err(|e| e.to_string())?;
    store
        .delete_merged_bill(identity_id, bill_id)
        .await
        .map_err(|e| e.to_string())
}

/// 更新指定账单的备注。
#[tauri::command]
pub async fn update_bill_notes(
    state: State<'_, AppState>,
    identity_id: i64,
    bill_id: i64,
    notes: Option<String>,
) -> Result<(), String> {
    tracing::debug!(
        "[Bill] update_bill_notes: bill_id={}, has_notes={}",
        bill_id,
        notes.is_some()
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db().clone();
    let translator = state.db_file_manager.create_position_translator();
    let store = BillStoreImpl::new(db_conn, "", identity_id, translator)
        .await
        .map_err(|e| e.to_string())?;
    store
        .update_bill_notes(bill_id, notes)
        .await
        .map_err(|e| e.to_string())
}

/// 重建指定身份的合并账单表。
///
/// 清空该身份下所有合并账单，再从原始账单表重新合并写入。
#[tauri::command]
pub async fn rebuild_merged_bills(
    state: State<'_, AppState>,
    identity_id: i64,
) -> Result<usize, String> {
    tracing::info!("[Bill] rebuild_merged_bills: identity_id={}", identity_id);

    let db = state.db_manager.read().await;
    let db_conn = db.db().clone();
    let translator = state.db_file_manager.create_position_translator();
    let store = BillStoreImpl::new(db_conn, "", identity_id, translator)
        .await
        .map_err(|e| e.to_string())?;

    let count = store
        .rebuild_merged_from_original(identity_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "[Bill] rebuild_merged_bills completed: identity_id={}, rebuilt {} records",
        identity_id,
        count
    );

    Ok(count)
}

/// 对指定身份的合并账单执行回填+去重。
///
/// 先回填缺失的元数据（如规范化交易号、位置信息），
/// 再按交易号去重，保留字段最完整的记录。
#[tauri::command]
pub async fn dedupe_identity_bills(
    state: State<'_, AppState>,
    identity_id: i64,
) -> Result<DedupeResult, String> {
    tracing::info!("[Bill] dedupe_identity_bills: identity_id={}", identity_id);

    let db = state.db_manager.read().await;
    let db_conn = db.db().clone();
    let translator = state.db_file_manager.create_position_translator();
    let store = BillStoreImpl::new(db_conn, "", identity_id, translator)
        .await
        .map_err(|e| e.to_string())?;

    let backfilled_count = store
        .backfill_merged_metadata(identity_id)
        .await
        .map_err(|e| e.to_string())?;
    let removed_count = store
        .dedupe_merged_by_identity(identity_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "[Bill] dedupe_identity_bills completed: backfilled={}, removed={}",
        backfilled_count,
        removed_count
    );

    Ok(DedupeResult {
        backfilled_count,
        removed_count,
    })
}

/// 对指定账号的原始账单执行回填+去重。
#[tauri::command]
pub async fn dedupe_account_bills(
    state: State<'_, AppState>,
    identity_id: i64,
    account_id: String,
) -> Result<DedupeResult, String> {
    tracing::info!(
        "[Bill] dedupe_account_bills: identity_id={}, account_id={}",
        identity_id,
        account_id
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db().clone();
    let translator = state.db_file_manager.create_position_translator();
    let store = BillStoreImpl::new(db_conn, &account_id, identity_id, translator)
        .await
        .map_err(|e| e.to_string())?;

    let backfilled_count = store
        .backfill_original_metadata(&account_id)
        .await
        .map_err(|e| e.to_string())?;
    let removed_count = store
        .dedupe_original_by_account(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "[Bill] dedupe_account_bills completed: backfilled={}, removed={}",
        backfilled_count,
        removed_count
    );

    Ok(DedupeResult {
        backfilled_count,
        removed_count,
    })
}

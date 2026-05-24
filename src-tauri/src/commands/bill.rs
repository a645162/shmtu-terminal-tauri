use serde::{Deserialize, Serialize};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use tauri::State;

use crate::db::BillStoreImpl;
use crate::entity::bill_merged;
use crate::models::BillMerged;
use crate::state::AppState;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillQueryResult {
    pub items: Vec<BillItemFrontend>,
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
}

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

#[tauri::command]
pub async fn query_bills(
    state: State<'_, AppState>,
    params: BillQueryParams,
) -> Result<BillQueryResult, String> {
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

    let (bills, _total_pages) = store
        .list_merged_bills(identity_id, params.page, params.page_size)
        .await
        .map_err(|e| e.to_string())?;

    let total = bill_merged::Entity::find()
        .filter(bill_merged::Column::IdentityId.eq(identity_id))
        .count(db.db())
        .await
        .unwrap_or(0) as u32;

    let items: Vec<BillItemFrontend> = bills.into_iter().map(BillItemFrontend::from).collect();

    Ok(BillQueryResult {
        items,
        total,
        page: params.page,
        page_size: params.page_size,
    })
}

#[tauri::command]
pub async fn delete_merged_bill(
    state: State<'_, AppState>,
    identity_id: i64,
    bill_id: i64,
) -> Result<(), String> {
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

#[tauri::command]
pub async fn update_bill_notes(
    state: State<'_, AppState>,
    identity_id: i64,
    bill_id: i64,
    notes: Option<String>,
) -> Result<(), String> {
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

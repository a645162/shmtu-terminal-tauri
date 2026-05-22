use tauri::State;

use crate::models::{Account, CreateAccountParams};
use crate::state::AppState;

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>, identity_id: i64) -> Result<Vec<Account>, String> {
    let db = state.db_manager.read().await;
    db.list_accounts_by_identity(identity_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_account(state: State<'_, AppState>, account: CreateAccountParams) -> Result<Account, String> {
    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;
    let id = db.create_account(&account, &crypto).map_err(|e| e.to_string())?;
    drop(crypto);
    let account = db.get_account(id).map_err(|e| e.to_string())?;
    account.ok_or_else(|| "创建账号后未找到".to_string())
}

#[tauri::command]
pub async fn update_account(state: State<'_, AppState>, account: Account) -> Result<(), String> {
    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;
    db.update_account(&account, &crypto).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_account(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = state.db_manager.read().await;
    db.delete_account(id).map_err(|e| e.to_string())
}

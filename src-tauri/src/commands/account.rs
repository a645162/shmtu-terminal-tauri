use tauri::State;

use crate::models::{Account, CreateAccountParams};
use crate::state::AppState;

/// 查询指定身份下的所有账号。
#[tauri::command]
pub async fn list_accounts(
    state: State<'_, AppState>,
    identity_id: i64,
) -> Result<Vec<Account>, String> {
    tracing::debug!(
        "[Account] list_accounts called, identity_id={}",
        identity_id
    );
    let db = state.db_manager.read().await;
    db.list_accounts_by_identity(identity_id)
        .await
        .map_err(|e| {
            tracing::error!("[Account] list_accounts FAILED: {}", e);
            e.to_string()
        })
        .map(|accounts| {
            tracing::debug!("[Account] list_accounts success, count={}", accounts.len());
            accounts
        })
}

/// 创建新账号。密码在存储前自动加密。
#[tauri::command]
pub async fn create_account(
    state: State<'_, AppState>,
    account: CreateAccountParams,
) -> Result<Account, String> {
    tracing::info!(
        "[Account] create_account called, account_id={}",
        account.account_id
    );
    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;
    let id = db.create_account(&account, &crypto).await.map_err(|e| {
        tracing::error!("[Account] create_account FAILED: {}", e);
        e.to_string()
    })?;
    drop(crypto);
    db.get_account(id)
        .await
        .map_err(|e| {
            tracing::error!("[Account] create_account: get_account FAILED: {}", e);
            e.to_string()
        })?
        .ok_or_else(|| {
            tracing::error!("[Account] create_account: account not found after create");
            "创建账号后未找到".to_string()
        })
        .map(|account| {
            tracing::info!("[Account] create_account success, id={}", account.id);
            account
        })
}

/// 更新账号信息。若密码字段为明文则自动加密后存储。
#[tauri::command]
pub async fn update_account(
    state: State<'_, AppState>,
    mut account: Account,
) -> Result<(), String> {
    tracing::info!("[Account] update_account called, id={}", account.id);
    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;

    // 判断密码是否为明文（未加密格式），若是则加密后存储
    if !account.password.is_empty() && !is_encrypted_format(&account.password) {
        account.password = crypto.encrypt_string(&account.password).map_err(|e| {
            tracing::error!("[Account] update_account: encrypt FAILED: {}", e);
            e.to_string()
        })?;
    }

    db.update_account(&account, &crypto)
        .await
        .map_err(|e| {
            tracing::error!("[Account] update_account FAILED: {}", e);
            e.to_string()
        })
        .map(|_| {
            tracing::info!("[Account] update_account success");
        })
}

/// 判断字符串是否为已加密格式（Base64 编码且解码后长度 >= 16 字节）。
///
/// 用于区分前端传入的明文密码和数据库中已有的加密密码。
fn is_encrypted_format(s: &str) -> bool {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    BASE64.decode(s).map(|d| d.len() >= 16).unwrap_or(false)
}

/// 删除指定账号及其关联数据（原始账单、会话信息）。
#[tauri::command]
pub async fn delete_account(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    tracing::warn!("[Account] delete_account called, id={}", id);
    let db = state.db_manager.read().await;
    db.delete_account(id)
        .await
        .map_err(|e| {
            tracing::error!("[Account] delete_account FAILED: {}", e);
            e.to_string()
        })
        .map(|_| {
            tracing::warn!("[Account] delete_account success, id={}", id);
        })
}

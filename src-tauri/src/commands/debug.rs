//! 调试用命令 (仅 debug 页面使用)
//!
//! 包含清除存储 cookies、清除 person_account 缓存等高风险操作, 方便开发期调试。

use tauri::State;

use crate::state::AppState;

/// 清除存储的所有账号 session cookies
///
/// 调用后下次拉取任何 epay 资源都会触发重新登录 (Manual/OCR/LocalOnnx)。
/// 同时清理该账号的 person_account_cache 缓存 (因为缓存里关联的 csrf_token 也会失效)。
#[tauri::command]
pub async fn clear_all_cookies(
    state: State<'_, AppState>,
) -> Result<ClearCookiesSummary, String> {
    let db = state.db_manager.read().await;

    // 1) 列出所有账号
    let identities = db
        .list_identities()
        .await
        .map_err(|e| format!("list_identities 失败: {}", e))?;

    let mut total_sessions = 0u32;
    let mut total_caches = 0u32;
    let mut accounts_visited = 0u32;

    for identity in identities {
        let accounts = db
            .list_accounts_by_identity(identity.id)
            .await
            .map_err(|e| format!("list_accounts 失败: {}", e))?;
        for account in accounts {
            accounts_visited += 1;
            // 删除 session
            if db
                .delete_session(&account.account_id)
                .await
                .is_ok()
            {
                total_sessions += 1;
            }
            // 删除 person_account_cache
            if db
                .delete_person_account_cache(&account.account_id)
                .await
                .is_ok()
            {
                total_caches += 1;
            }
        }
    }

    tracing::warn!(
        "[Debug] clear_all_cookies: visited={}, sessions_cleared={}, caches_cleared={}",
        accounts_visited, total_sessions, total_caches
    );

    Ok(ClearCookiesSummary {
        accounts_visited,
        sessions_cleared: total_sessions,
        caches_cleared: total_caches,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ClearCookiesSummary {
    pub accounts_visited: u32,
    pub sessions_cleared: u32,
    pub caches_cleared: u32,
}

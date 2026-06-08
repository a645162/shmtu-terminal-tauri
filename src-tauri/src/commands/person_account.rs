//! Tauri commands for 一卡通个人账户详情（CAS /epay/personaccount/index）
//!
//! 登录流程复用账单同步那套: 前端调 sync 命令 → session 过期时 sync 自动触发
//! Manual/Ocr 登录 → 成功后 cookies 自动保存到 session 表。
//! 这里的 person_account 命令只负责 **用已有 cookies 拉取 + 解析 + 缓存**。
//! 如果 cookies 过期, 返回 SESSION_EXPIRED 错误,
//! 前端 UI 提示用户"请先同步账单/重新登录"。

use shmtu_cas::cas::epay::{EpayAuth, LoginProbe};
use shmtu_cas::parser::person_account::parse_person_account;
use tauri::State;

use crate::models::PersonAccountInfo;
use crate::state::AppState;

/// 拉取并解析一卡通个人账户详情（仅用已有 session, 不过期自动重新登录）。
///
/// 登录由账单同步流程负责, 这里只消费已有 cookies。
#[tauri::command]
pub async fn fetch_person_account(
    state: State<'_, AppState>,
    account_db_id: i64,
) -> Result<PersonAccountInfo, String> {
    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;

    let account = db
        .get_account(account_db_id)
        .await
        .map_err(|e| format!("{}", e))?
        .ok_or_else(|| "账号不存在".to_string())?;

    // 1) 恢复已有 session
    let session = db
        .get_session(&account.account_id, &crypto)
        .await
        .map_err(|e| format!("{}", e))?
        .ok_or_else(|| "SESSION_EXPIRED: 未找到保存的会话, 请先同步账单或登录".to_string())?;

    let mut epay = EpayAuth::new().map_err(|e| format!("创建EpayAuth失败: {}", e))?;
    epay
        .restore_session(&session.cookies)
        .map_err(|e| format!("恢复会话失败: {}", e))?;

    if !matches!(epay.probe_login().await, Ok(LoginProbe::AlreadyLoggedIn)) {
        return Err("SESSION_EXPIRED: 会话已失效, 请重新同步账单以刷新 cookies".to_string());
    }

    // 2) 拉取 HTML
    let html = epay
        .get_person_account_html()
        .await
        .map_err(|e| format!("拉取个人账户页面失败: {}", e))?;

    // 3) 解析
    let parsed =
        parse_person_account(&html).map_err(|e| format!("解析个人账户页面失败: {}", e))?;

    // 4) 落库并返回
    let now = chrono::Utc::now().to_rfc3339();
    let info = PersonAccountInfo {
        account_id: account.account_id.clone(),
        real_name: parsed.real_name,
        real_name_auth_status: parsed.real_name_auth_status,
        cash_balance: parsed.cash_balance,
        cash_balance_raw: parsed.cash_balance_raw,
        security_question_status: parsed.security_question_status,
        register_date: parsed.register_date,
        student_id: parsed.student_id,
        email: parsed.email,
        nickname: parsed.nickname,
        gender: parsed.gender,
        class_name: parsed.class_name,
        phone_num: parsed.phone_num,
        id_type: parsed.id_type,
        id_number: parsed.id_number,
        remark: parsed.remark,
        user_type: parsed.user_type,
        csrf_token: parsed.csrf_token,
        csrf_header: parsed.csrf_header,
        fetched_at: now,
    };

    db.upsert_person_account_cache(&info)
        .await
        .map_err(|e| format!("{}", e))?;

    tracing::info!(
        "[PersonAccount] success, balance={}",
        info.cash_balance_raw
    );
    Ok(info)
}

/// 从缓存读取个人账户详情（不发起网络请求）。
#[tauri::command]
pub async fn get_cached_person_account(
    state: State<'_, AppState>,
    account_db_id: i64,
) -> Result<Option<PersonAccountInfo>, String> {
    let db = state.db_manager.read().await;
    let account = match db.get_account(account_db_id).await.map_err(|e| e.to_string())? {
        Some(a) => a,
        None => return Ok(None),
    };
    db.get_person_account_cache(&account.account_id)
        .await
        .map_err(|e| e.to_string())
}

/// 按账号主键列表读取缓存（用于 IdentityManagerDialog 展开时一次性拉取）。
#[tauri::command]
pub async fn list_cached_person_accounts(
    state: State<'_, AppState>,
    account_db_ids: Vec<i64>,
) -> Result<Vec<PersonAccountInfo>, String> {
    if account_db_ids.is_empty() {
        return Ok(Vec::new());
    }
    let db = state.db_manager.read().await;
    let mut student_ids: Vec<String> = Vec::with_capacity(account_db_ids.len());
    for id in account_db_ids {
        if let Some(acct) = db.get_account(id).await.map_err(|e| e.to_string())? {
            student_ids.push(acct.account_id);
        }
    }
    db.list_person_account_caches_by_ids(&student_ids)
        .await
        .map_err(|e| e.to_string())
}

//! Tauri commands for 一卡通个人账户详情（CAS /epay/personaccount/index）

use shmtu_cas::cas::epay::{EpayAuth, LoginProbe};
use shmtu_cas::parser::person_account::parse_person_account;
use tauri::State;

use crate::models::PersonAccountInfo;
use crate::state::AppState;

/// 拉取并解析一卡通个人账户详情，结果写入缓存后返回。
///
/// 流程：从数据库取回账号 + 会话 cookies，恢复 EpayAuth 会话后请求
/// /epay/personaccount/index，解析 HTML 写入 person_account_cache。
#[tauri::command]
pub async fn fetch_person_account(
    state: State<'_, AppState>,
    account_db_id: i64,
) -> Result<PersonAccountInfo, String> {
    tracing::info!(
        "[PersonAccount] fetch_person_account called, account_db_id={}",
        account_db_id
    );

    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;

    // 1) 拿到账号（按主键）
    let account = db
        .get_account(account_db_id)
        .await
        .map_err(|e| {
            tracing::error!("[PersonAccount] get_account FAILED: {}", e);
            e.to_string()
        })?
        .ok_or_else(|| "账号不存在".to_string())?;

    // 2) 取会话
    let session = db
        .get_session(&account.account_id, &crypto)
        .await
        .map_err(|e| {
            tracing::error!("[PersonAccount] get_session FAILED: {}", e);
            e.to_string()
        })?
        .ok_or_else(|| "账号未登录，请先完成登录".to_string())?;

    // 3) 恢复 EpayAuth 会话
    let mut epay = EpayAuth::new().map_err(|e| {
        tracing::error!("[PersonAccount] EpayAuth::new FAILED: {}", e);
        format!("创建EpayAuth失败: {}", e)
    })?;
    epay.restore_session(&session.cookies).map_err(|e| {
        tracing::error!("[PersonAccount] restore_session FAILED: {}", e);
        format!("恢复会话失败: {}", e)
    })?;

    // 4) 探测登录态
    match epay.probe_login().await {
        Ok(LoginProbe::AlreadyLoggedIn) => {}
        Ok(LoginProbe::NeedLogin { .. }) => {
            tracing::warn!("[PersonAccount] session invalid for {}", account.account_id);
            return Err("账号会话已失效，请重新登录".to_string());
        }
        Err(e) => {
            tracing::error!("[PersonAccount] probe_login FAILED: {}", e);
            return Err(format!("探测登录态失败: {}", e));
        }
    }

    // 5) 拉取 HTML
    let html = epay.get_person_account_html().await.map_err(|e| {
        tracing::error!("[PersonAccount] get_person_account_html FAILED: {}", e);
        format!("拉取个人账户页面失败: {}", e)
    })?;

    // 6) 解析
    let parsed = parse_person_account(&html).map_err(|e| {
        tracing::error!("[PersonAccount] parse_person_account FAILED: {}", e);
        format!("解析个人账户页面失败: {}", e)
    })?;

    // 7) 落库
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

    db.upsert_person_account_cache(&info).await.map_err(|e| {
        tracing::error!("[PersonAccount] upsert_person_account_cache FAILED: {}", e);
        e.to_string()
    })?;

    tracing::info!(
        "[PersonAccount] fetch_person_account success, account_id={}, cash_balance={}",
        info.account_id,
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

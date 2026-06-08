//! Tauri commands for 一卡通个人账户详情（CAS /epay/personaccount/index）
//!
//! 当 cookies 过期或不存在时, **仅执行登录流程获取 session** (不拉取账单),
//! 然后拉取 /epay/personaccount/index 获取余额与个人信息。
//!
//! 支持所有 CaptchaMode:
//! - RemoteOcr / RemoteOcrHttp / LocalOnnx → 自动登录, 无需用户交互
//! - Manual → 返回 MANUAL_CAPTCHA_REQUIRED, 前端弹出验证码输入框,
//!   用户输入后调用 submit_person_account_captcha 完成登录并拉取余额

use base64::Engine;
use serde::Serialize;
use shmtu_cas::captcha::CaptchaResolver;
use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use shmtu_cas::parser::person_account::parse_person_account;
use tauri::State;

use crate::config::CaptchaMode;
use crate::models::{Account, PersonAccountInfo};
use crate::state::AppState;
use crate::sync::ConfigAccess;

const BASE64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// 手动验证码模式: 后端无法自动识别验证码, 需前端弹窗让用户输入。
/// 前端收到此错误后弹出 CaptchaDialog, 用户输入后调用 submit_person_account_captcha。
#[derive(Debug, Clone, Serialize)]
pub struct ManualCaptchaRequired {
    pub image: String,
    pub execution: String,
}

/// 拉取并解析一卡通个人账户详情（session 过期/不存在时自动登录, 仅做登录不拉账单）。
#[tauri::command]
pub async fn fetch_person_account(
    state: State<'_, AppState>,
    account_db_id: i64,
) -> Result<PersonAccountInfo, String> {
    tracing::info!("[PersonAccount] fetch_person_account called, account_db_id={}", account_db_id);

    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;

    let account = db
        .get_account(account_db_id)
        .await
        .map_err(|e| format!("{}", e))?
        .ok_or_else(|| "账号不存在".to_string())?;

    // 1) 获取 EpayAuth (有 session 则恢复, 过期/没有则执行登录)
    let epay = get_or_login_epay(&*db, &*crypto, &account).await?;

    // 2) 保存登录后的 cookies (如果执行了登录)
    if let Ok(json) = epay.extract_session() {
        let _ = db.save_session(&account.account_id, &json, &crypto).await;
    }

    // 3) 拉取 HTML
    let html = epay
        .get_person_account_html()
        .await
        .map_err(|e| format!("拉取个人账户页面失败: {}", e))?;

    // 4) 解析
    let parsed =
        parse_person_account(&html).map_err(|e| format!("解析个人账户页面失败: {}", e))?;

    // 5) 落库并返回
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
        gender_from_id: shmtu_cas::parser::person_account::guess_gender_from_id_number(&parsed.id_number).to_string(),
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

    tracing::info!("[PersonAccount] success, balance={}", info.cash_balance_raw);
    Ok(info)
}

/// 获取已认证的 EpayAuth（优先恢复已有 session; 过期/不存在时执行仅登录, 不拉账单）。
async fn get_or_login_epay(
    db: &crate::db::DatabaseManager,
    crypto: &crate::crypto::CryptoService,
    account: &Account,
) -> Result<EpayAuth, String> {
    // 先尝试恢复已有 session
    let session = db.get_session(&account.account_id, crypto).await.map_err(|e| format!("{}", e))?;
    if let Some(s) = &session {
        let mut epay = EpayAuth::new().map_err(|e| format!("创建EpayAuth失败: {}", e))?;
        if epay.restore_session(&s.cookies).is_ok()
            && matches!(epay.probe_login().await, Ok(LoginProbe::AlreadyLoggedIn))
        {
            tracing::info!("[PersonAccount] session valid, reusing");
            return Ok(epay);
        }
        tracing::info!("[PersonAccount] session expired, performing login");
    } else {
        tracing::info!("[PersonAccount] no saved session, performing login");
    }

    // 执行登录 (不拉账单)
    let password = db
        .decrypt_account_password(account, crypto)
        .map_err(|e| format!("解密密码失败: {}", e))?;

    let cfg = ConfigAccess::new(db);

    match cfg.captcha_mode() {
        CaptchaMode::RemoteOcr | CaptchaMode::RemoteOcrHttp => {
            login_auto(account, &password, &cfg).await
        }
        CaptchaMode::LocalOnnx => {
            // LocalOnnx 无法在 server 端直接调用 (需要前端的 onnx 模型).
            // 这里退化为手动模式: 返回验证码让前端处理.
            login_manual_challenge(account).await
        }
        CaptchaMode::Manual => {
            login_manual_challenge(account).await
        }
    }
}

/// OCR 自动登录 (不拉账单, 仅获取 EpayAuth session)。
async fn login_auto(account: &Account, password: &str, cfg: &ConfigAccess) -> Result<EpayAuth, String> {
    let max_attempts = cfg.ocr_retry_count().max(1);

    for attempt in 1..=max_attempts {
        tracing::info!("[PersonAccount] Auto login attempt {}/{}", attempt, max_attempts);
        let mut fresh = EpayAuth::new().map_err(|e| format!("创建EpayAuth失败: {}", e))?;
        fresh.probe_login().await.map_err(|e| format!("探测登录态失败: {}", e))?;

        let challenge = fresh.prepare_challenge().await.map_err(|e| format!("获取验证码失败: {}", e))?;

        let captcha_code = match cfg.captcha_mode() {
            CaptchaMode::RemoteOcr => {
                shmtu_cas::captcha::OcrCaptchaResolver::new(&cfg.remote_ocr_host(), cfg.remote_ocr_port())
                    .with_retries(max_attempts)
                    .resolve(&challenge.captcha_image).await
                    .map_err(|e| format!("OCR失败: {}", e))?
                    .into_final_answer()
            }
            CaptchaMode::RemoteOcrHttp => {
                shmtu_cas::captcha::OcrHttpCaptchaResolver::new(&cfg.remote_ocr_http_url())
                    .with_retries(max_attempts)
                    .resolve(&challenge.captcha_image).await
                    .map_err(|e| format!("OCR失败: {}", e))?
                    .into_final_answer()
            }
            _ => unreachable!("login_auto called for non-OCR mode"),
        };

        tracing::info!("[PersonAccount] Captcha: {}", captcha_code);

        match fresh.submit_login(&account.account_id, password, &captcha_code, &challenge.execution)
            .await.map_err(|e| format!("提交登录失败: {}", e))?
        {
            LoginSubmitResult::Success => {
                if matches!(fresh.probe_login().await, Ok(LoginProbe::AlreadyLoggedIn)) {
                    tracing::info!("[PersonAccount] Auto login SUCCESS");
                    return Ok(fresh);
                }
            }
            LoginSubmitResult::ValidateCodeError => {
                if attempt < max_attempts { continue; }
            }
            LoginSubmitResult::PasswordError => return Err("用户名或密码错误".to_string()),
            LoginSubmitResult::Failure(msg) => {
                tracing::warn!("[PersonAccount] Login failed: {}", msg);
                if attempt < max_attempts { continue; }
            }
        }
    }
    Err("登录重试次数耗尽".to_string())
}

/// 手动验证码模式: 获取 challenge, 将图片+execution 返回给前端弹窗。
async fn login_manual_challenge(_account: &Account) -> Result<EpayAuth, String> {
    let mut epay = EpayAuth::new().map_err(|e| format!("创建EpayAuth失败: {}", e))?;
    epay.probe_login().await.map_err(|e| format!("探测登录态失败: {}", e))?;
    let challenge = epay.prepare_challenge().await.map_err(|e| format!("获取验证码失败: {}", e))?;
    let image = BASE64.encode(&challenge.captcha_image);

    Err(format!("MANUAL_CAPTCHA_REQUIRED|{}|{}", image, challenge.execution))
}

/// 提交手动验证码答案, 完成登录并拉取个人账户详情 (不拉账单)。
#[tauri::command]
pub async fn submit_person_account_captcha(
    state: State<'_, AppState>,
    account_db_id: i64,
    captcha_code: String,
    execution: String,
) -> Result<PersonAccountInfo, String> {
    let db = state.db_manager.read().await;
    let crypto = state.crypto.read().await;

    let account = db
        .get_account(account_db_id)
        .await.map_err(|e| format!("{}", e))?
        .ok_or_else(|| "账号不存在".to_string())?;

    let password = db
        .decrypt_account_password(&account, &crypto)
        .map_err(|e| format!("解密密码失败: {}", e))?;

    let mut epay = EpayAuth::new().map_err(|e| format!("创建EpayAuth失败: {}", e))?;
    epay.probe_login().await.map_err(|e| format!("探测登录态失败: {}", e))?;

    match epay
        .submit_login(&account.account_id, &password, &captcha_code, &execution)
        .await.map_err(|e| format!("提交登录失败: {}", e))?
    {
        LoginSubmitResult::Success => {
            if !matches!(epay.probe_login().await, Ok(LoginProbe::AlreadyLoggedIn)) {
                return Err("登录验证失败".to_string());
            }
        }
        LoginSubmitResult::ValidateCodeError => return Err("VALIDATE_CODE_ERROR".to_string()),
        LoginSubmitResult::PasswordError => return Err("PASSWORD_ERROR".to_string()),
        LoginSubmitResult::Failure(msg) => return Err(format!("登录失败: {}", msg)),
    }

    // 保存 cookies
    if let Ok(json) = epay.extract_session() {
        let _ = db.save_session(&account.account_id, &json, &crypto).await;
    }

    // 拉取 HTML
    let html = epay
        .get_person_account_html()
        .await.map_err(|e| format!("拉取个人账户页面失败: {}", e))?;
    let parsed = parse_person_account(&html).map_err(|e| format!("解析个人账户页面失败: {}", e))?;

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
        gender_from_id: shmtu_cas::parser::person_account::guess_gender_from_id_number(&parsed.id_number).to_string(),
        id_type: parsed.id_type,
        id_number: parsed.id_number,
        remark: parsed.remark,
        user_type: parsed.user_type,
        csrf_token: parsed.csrf_token,
        csrf_header: parsed.csrf_header,
        fetched_at: now,
    };
    db.upsert_person_account_cache(&info).await.map_err(|e| format!("{}", e))?;

    tracing::info!("[PersonAccount] submit_captcha success, balance={}", info.cash_balance_raw);
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
    db.get_person_account_cache(&account.account_id).await.map_err(|e| e.to_string())
}

/// 按账号主键列表读取缓存（用于 IdentityManagerDialog 展开时一次性拉取）。
#[tauri::command]
pub async fn list_cached_person_accounts(
    state: State<'_, AppState>,
    account_db_ids: Vec<i64>,
) -> Result<Vec<PersonAccountInfo>, String> {
    if account_db_ids.is_empty() { return Ok(Vec::new()); }
    let db = state.db_manager.read().await;
    let mut student_ids: Vec<String> = Vec::with_capacity(account_db_ids.len());
    for id in account_db_ids {
        if let Some(acct) = db.get_account(id).await.map_err(|e| e.to_string())? {
            student_ids.push(acct.account_id);
        }
    }
    db.list_person_account_caches_by_ids(&student_ids).await.map_err(|e| e.to_string())
}

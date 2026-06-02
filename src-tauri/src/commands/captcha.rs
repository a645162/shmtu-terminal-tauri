use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use shmtu_cas::captcha::CaptchaResolver;
use shmtu_ocr::backend::CasOnnxBackend;
use tauri::State;

use crate::config::CaptchaMode;
use crate::state::{AppState, CaptchaTestSession};

/// 验证码测试结果（前端展示用）
#[derive(Debug, Clone, Serialize)]
pub struct CaptchaTestResultFrontend {
    pub id: u32,
    pub success: bool,
    pub expression: String,
    pub answer: String,
    pub duration_ms: u64,
    pub mode: String,
    pub verification: Option<String>,
    pub error: Option<String>,
    pub captcha_image: Option<String>,
}

const CAPTCHA_TEST_PROBE_USERNAME: &str = "__captcha_test_invalid_user__";
const CAPTCHA_TEST_PROBE_PASSWORD: &str = "__captcha_test_invalid_password__";

async fn verify_captcha_answer(
    epay: &shmtu_cas::cas::epay::EpayAuth,
    execution: &str,
    answer: &str,
) -> Result<(bool, Option<String>, Option<String>), String> {
    use shmtu_cas::cas::epay::LoginSubmitResult;

    match epay
        .submit_login(
            CAPTCHA_TEST_PROBE_USERNAME,
            CAPTCHA_TEST_PROBE_PASSWORD,
            answer,
            execution,
        )
        .await
        .map_err(|e| format!("提交验证码校验失败: {}", e))?
    {
        LoginSubmitResult::PasswordError => {
            Ok((true, Some("密码错误，说明验证码正确".to_string()), None))
        }
        LoginSubmitResult::ValidateCodeError => Ok((
            false,
            Some("验证码错误".to_string()),
            Some("验证码错误".to_string()),
        )),
        LoginSubmitResult::Success => Ok((true, Some("登录成功，验证码正确".to_string()), None)),
        LoginSubmitResult::Failure(msg) => Ok((
            false,
            Some(format!("登录返回异常: {}", msg)),
            Some(format!("登录探测失败: {}", msg)),
        )),
    }
}

async fn do_manual_test_captcha(
    state: &AppState,
    answer: &str,
    duration_ms: u64,
) -> Result<CaptchaTestResultFrontend, String> {
    use shmtu_cas::cas::epay::LoginSubmitResult;

    let mut session_guard = state.captcha_test_session.lock().await;
    let pending = session_guard
        .as_mut()
        .ok_or_else(|| "请先刷新验证码，再输入当前图片中的答案".to_string())?;

    match pending
        .epay
        .submit_login(
            CAPTCHA_TEST_PROBE_USERNAME,
            CAPTCHA_TEST_PROBE_PASSWORD,
            answer,
            &pending.execution,
        )
        .await
        .map_err(|e| format!("提交验证码校验失败: {}", e))?
    {
        LoginSubmitResult::PasswordError => {
            *session_guard = None;
            Ok(CaptchaTestResultFrontend {
                id: 1,
                success: true,
                expression: String::new(),
                answer: answer.to_string(),
                duration_ms,
                mode: "manual".to_string(),
                verification: Some("密码错误，说明验证码正确".to_string()),
                error: None,
                captcha_image: None,
            })
        }
        LoginSubmitResult::ValidateCodeError => {
            let challenge = pending
                .epay
                .prepare_challenge()
                .await
                .map_err(|e| format!("刷新验证码失败: {}", e))?;
            let image = BASE64.encode(&challenge.captcha_image);
            pending.execution = challenge.execution;
            Ok(CaptchaTestResultFrontend {
                id: 1,
                success: false,
                expression: String::new(),
                answer: answer.to_string(),
                duration_ms,
                mode: "manual".to_string(),
                verification: Some("验证码错误".to_string()),
                error: Some("验证码错误".to_string()),
                captcha_image: Some(image),
            })
        }
        LoginSubmitResult::Success => {
            *session_guard = None;
            Ok(CaptchaTestResultFrontend {
                id: 1,
                success: true,
                expression: String::new(),
                answer: answer.to_string(),
                duration_ms,
                mode: "manual".to_string(),
                verification: Some("登录成功，验证码正确".to_string()),
                error: None,
                captcha_image: None,
            })
        }
        LoginSubmitResult::Failure(msg) => Ok(CaptchaTestResultFrontend {
            id: 1,
            success: false,
            expression: String::new(),
            answer: answer.to_string(),
            duration_ms,
            mode: "manual".to_string(),
            verification: Some(format!("登录返回异常: {}", msg)),
            error: Some(format!("登录探测失败: {}", msg)),
            captcha_image: None,
        }),
    }
}

/// 获取验证码图片（base64 编码）。
///
/// 流程：探测登录状态 -> 若需登录则获取验证码挑战图片。
/// 若已登录则返回错误提示"已登录，无需验证码"。
#[tauri::command]
pub async fn get_captcha_image() -> Result<String, String> {
    tracing::debug!("[Captcha] get_captcha_image called");

    use shmtu_cas::cas::epay::EpayAuth;

    let mut epay = EpayAuth::new().map_err(|e| {
        tracing::error!("[Captcha] get_captcha_image: EpayAuth::new() failed: {}", e);
        format!("创建EpayAuth失败: {}", e)
    })?;

    match epay.probe_login().await {
        Ok(shmtu_cas::cas::epay::LoginProbe::AlreadyLoggedIn) => {
            tracing::info!("[Captcha] get_captcha_image: already logged in");
            Err("已登录，无需验证码".to_string())
        }
        Ok(shmtu_cas::cas::epay::LoginProbe::NeedLogin { .. }) => {
            match epay.prepare_challenge().await {
                Ok(challenge) => {
                    let base64_image = BASE64.encode(&challenge.captcha_image);
                    tracing::info!(
                        "[Captcha] get_captcha_image success, image_len={}",
                        base64_image.len()
                    );
                    Ok(base64_image)
                }
                Err(e) => {
                    tracing::error!(
                        "[Captcha] get_captcha_image: prepare_challenge failed: {}",
                        e
                    );
                    Err(format!("获取验证码失败: {}", e))
                }
            }
        }
        Err(e) => {
            tracing::error!("[Captcha] get_captcha_image: probe_login failed: {}", e);
            Err(format!("探测登录状态失败: {}", e))
        }
    }
}

/// 验证码挑战响应，包含图片和 execution 参数。
///
/// execution 参数用于 CAS 登录流程的后续步骤。
#[derive(Debug, Clone, Serialize)]
pub struct CaptchaChallengeResponse {
    pub captcha_image: String,
    pub execution: String,
}

/// 获取验证码图片及 execution 参数。
///
/// 与 `get_captcha_image` 类似，但额外返回 CAS 登录所需的 execution 字段，
/// 供前端在自动登录流程中使用。
#[tauri::command]
pub async fn get_captcha_with_execution(
    state: State<'_, AppState>,
) -> Result<CaptchaChallengeResponse, String> {
    tracing::debug!("[Captcha] get_captcha_with_execution called");

    use shmtu_cas::cas::epay::EpayAuth;

    let mut epay = EpayAuth::new().map_err(|e| {
        tracing::error!(
            "[Captcha] get_captcha_with_execution: EpayAuth::new() failed: {}",
            e
        );
        format!("创建EpayAuth失败: {}", e)
    })?;

    match epay.probe_login().await {
        Ok(shmtu_cas::cas::epay::LoginProbe::AlreadyLoggedIn) => {
            tracing::info!("[Captcha] get_captcha_with_execution: already logged in");
            Err("已登录，无需验证码".to_string())
        }
        Ok(shmtu_cas::cas::epay::LoginProbe::NeedLogin { .. }) => {
            match epay.prepare_challenge().await {
                Ok(challenge) => {
                    let base64_image = BASE64.encode(&challenge.captcha_image);
                    tracing::info!("[Captcha] get_captcha_with_execution success");
                    let execution = challenge.execution;
                    let mut session = state.captcha_test_session.lock().await;
                    *session = Some(CaptchaTestSession {
                        epay,
                        execution: execution.clone(),
                    });
                    Ok(CaptchaChallengeResponse {
                        captcha_image: base64_image,
                        execution,
                    })
                }
                Err(e) => {
                    tracing::error!(
                        "[Captcha] get_captcha_with_execution: prepare_challenge failed: {}",
                        e
                    );
                    Err(format!("获取验证码失败: {}", e))
                }
            }
        }
        Err(e) => {
            tracing::error!(
                "[Captcha] get_captcha_with_execution: probe_login failed: {}",
                e
            );
            Err(format!("探测登录状态失败: {}", e))
        }
    }
}

/// 内部实现：获取验证码并尝试识别。
///
/// 支持三种模式：
/// - `manual`：仅获取验证码图片，不自动识别，返回"手动模式需要用户输入"
/// - `remote_ocr`：通过远程 OCR 服务识别验证码表达式并计算答案
/// - `local_onnx`：本地 ONNX 模型识别（暂未实现）
async fn do_test_captcha(
    state: Option<&AppState>,
    mode: &str,
    manual_input: Option<&str>,
) -> Result<CaptchaTestResultFrontend, String> {
    tracing::debug!("[Captcha] do_test_captcha called, mode={}", mode);

    let captcha_mode = match mode {
        "manual" => CaptchaMode::Manual,
        "remote_ocr" => CaptchaMode::RemoteOcr,
        "remote_ocr_http" => CaptchaMode::RemoteOcrHttp,
        "local_onnx" => CaptchaMode::LocalOnnx,
        _ => {
            tracing::error!("[Captcha] do_test_captcha: unknown mode: {}", mode);
            return Err(format!("未知验证码模式: {}", mode));
        }
    };

    let start = std::time::Instant::now();

    if matches!(captcha_mode, CaptchaMode::Manual) {
        let answer = manual_input
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "手动模式请输入验证码".to_string())?;
        let state_ref = state.ok_or_else(|| "手动模式需要应用状态".to_string())?;
        return do_manual_test_captcha(state_ref, answer, start.elapsed().as_millis() as u64).await;
    }

    use shmtu_cas::cas::epay::EpayAuth;
    let mut epay = EpayAuth::new().map_err(|e| {
        tracing::error!("[Captcha] do_test_captcha: EpayAuth::new() failed: {}", e);
        format!("创建EpayAuth失败: {}", e)
    })?;

    match epay.probe_login().await.map_err(|e| {
        tracing::error!("[Captcha] do_test_captcha: probe_login failed: {}", e);
        format!("探测登录状态失败: {}", e)
    })? {
        shmtu_cas::cas::epay::LoginProbe::AlreadyLoggedIn => {
            return Err("当前已登录，无需验证码测试；请先清理会话后再试".to_string());
        }
        shmtu_cas::cas::epay::LoginProbe::NeedLogin { .. } => {}
    }

    let challenge = epay.prepare_challenge().await.map_err(|e| {
        tracing::error!("[Captcha] do_test_captcha: prepare_challenge failed: {}", e);
        format!("获取验证码失败: {}", e)
    })?;

    let recognition = match captcha_mode {
        CaptchaMode::Manual => unreachable!("manual mode handled above"),
        CaptchaMode::RemoteOcr => {
            let (host, port, retry_count) = if let Some(state) = state {
                let config = state.config.read().await;
                let captcha_config = &config.get().captcha;
                (
                    captcha_config.remote_ocr_host.clone(),
                    captcha_config.remote_ocr_port,
                    captcha_config.ocr_retry_count,
                )
            } else {
                (String::new(), 0, 3)
            };

            if host.is_empty() || port == 0 {
                tracing::error!("[Captcha] do_test_captcha: remote OCR (TCP) not configured");
                Err("未配置远程OCR服务器地址".to_string())
            } else {
                tracing::info!(
                    "[Captcha] do_test_captcha: using remote OCR (TCP) {}:{}",
                    host,
                    port
                );
                let resolver = shmtu_cas::captcha::OcrCaptchaResolver::new(&host, port)
                    .with_retries(retry_count);
                match resolver.resolve(&challenge.captcha_image).await {
                    Ok(result) => {
                        let expr = result.value.clone();
                        let ans = result.into_final_answer();
                        tracing::info!(
                            "[Captcha] do_test_captcha: OCR (TCP) success, answer={}",
                            ans
                        );
                        Ok((expr, ans))
                    }
                    Err(e) => {
                        tracing::error!("[Captcha] do_test_captcha: OCR (TCP) failed: {}", e);
                        Err(format!("远程OCR识别失败: {}", e))
                    }
                }
            }
        }
        CaptchaMode::RemoteOcrHttp => {
            let (http_url, retry_count) = if let Some(state) = state {
                let config = state.config.read().await;
                let captcha_config = &config.get().captcha;
                (
                    captcha_config.remote_ocr_http_url.clone(),
                    captcha_config.ocr_retry_count,
                )
            } else {
                (String::new(), 3)
            };

            if http_url.is_empty() {
                tracing::error!("[Captcha] do_test_captcha: RESTful OCR URL not configured");
                Err("未配置RESTful OCR服务器地址".to_string())
            } else {
                tracing::info!(
                    "[Captcha] do_test_captcha: using remote OCR (RESTful) {}",
                    http_url
                );
                let resolver = shmtu_cas::captcha::OcrHttpCaptchaResolver::new(&http_url)
                    .with_retries(retry_count);
                match resolver.resolve(&challenge.captcha_image).await {
                    Ok(result) => {
                        let expr = result.value.clone();
                        let ans = result.into_final_answer();
                        tracing::info!(
                            "[Captcha] do_test_captcha: OCR (RESTful) success, answer={}",
                            ans
                        );
                        Ok((expr, ans))
                    }
                    Err(e) => {
                        tracing::error!("[Captcha] do_test_captcha: OCR (RESTful) failed: {}", e);
                        Err(format!("RESTful OCR识别失败: {}", e))
                    }
                }
            }
        }
        CaptchaMode::LocalOnnx => {
            let state_ref = state.ok_or_else(|| "LocalOnnx模式需要应用状态".to_string())?;
            let local_ocr = state_ref.local_ocr.clone();

            // 检查是否已初始化，未初始化则尝试加载模型
            let needs_load = {
                let guard = local_ocr
                    .lock()
                    .map_err(|e| format!("获取ONNX锁失败: {}", e))?;
                guard.is_none()
            };
            // guard 已释放，可以安全 await

            if needs_load {
                let config = state_ref.config.read().await;
                let model_path = config.onnx_model_path();
                let missing = CasOnnxBackend::missing_model_files(&model_path);
                if !missing.is_empty() {
                    let missing_str = missing.join(", ");
                    tracing::error!(
                        "[Captcha] do_test_captcha: ONNX模型文件不完整，缺少: {}",
                        missing_str
                    );
                    return Err(format!("ONNX模型文件不完整，缺少: {}", missing_str));
                }
                tracing::info!(
                    "[Captcha] do_test_captcha: loading ONNX models from {:?}",
                    model_path
                );
                let backend = CasOnnxBackend::load(&model_path).map_err(|e| {
                    tracing::error!("[Captcha] do_test_captcha: 加载ONNX模型失败: {}", e);
                    format!("加载ONNX模型失败: {}", e)
                })?;
                let mut guard = local_ocr
                    .lock()
                    .map_err(|e| format!("获取ONNX锁失败: {}", e))?;
                *guard = Some(backend);
                tracing::info!("[Captcha] do_test_captcha: ONNX models loaded successfully");
            }

            // 使用 ONNX 推理（CPU 密集操作，spawn_blocking）
            let image_data = challenge.captcha_image.clone();
            let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
                let mut guard = local_ocr
                    .lock()
                    .map_err(|e| format!("获取ONNX锁失败: {}", e))?;
                let backend = guard
                    .as_mut()
                    .ok_or_else(|| "ONNX后端未初始化".to_string())?;
                backend
                    .predict_bytes(&image_data)
                    .map(|r| r.expr)
                    .map_err(|e| format!("ONNX推理失败: {}", e))
            })
            .await
            .map_err(|e| format!("ONNX任务执行失败: {}", e))?;

            match result {
                Ok(expr) => {
                    let ans = shmtu_cas::captcha::get_expr_result(&expr);
                    tracing::info!(
                        "[Captcha] do_test_captcha: Local ONNX success, expr={}, answer={}",
                        expr,
                        ans
                    );
                    Ok((expr, ans))
                }
                Err(e) => {
                    tracing::error!("[Captcha] do_test_captcha: Local ONNX failed: {}", e);
                    Err(format!("本地ONNX识别失败: {}", e))
                }
            }
        }
    };

    let (expression, answer, success, verification, error) = match recognition {
        Ok((expression, answer)) => {
            let (success, verification, error) =
                verify_captcha_answer(&epay, &challenge.execution, &answer).await?;
            (expression, answer, success, verification, error)
        }
        Err(error) => (String::new(), String::new(), false, None, Some(error)),
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(CaptchaTestResultFrontend {
        id: 1,
        success,
        expression,
        answer,
        duration_ms,
        mode: mode.to_string(),
        verification,
        error,
        captcha_image: None,
    })
}

/// 单次验证码识别测试。
///
/// 获取验证码图片后尝试用指定模式识别，返回识别结果和耗时。
#[tauri::command]
pub async fn test_captcha(
    state: State<'_, AppState>,
    mode: String,
    manual_input: Option<String>,
) -> Result<CaptchaTestResultFrontend, String> {
    tracing::info!("[Captcha] test_captcha called, mode={}", mode);
    match do_test_captcha(Some(&state), &mode, manual_input.as_deref()).await {
        Ok(result) => {
            tracing::info!("[Captcha] test_captcha success");
            Ok(result)
        }
        Err(e) => {
            tracing::error!("[Captcha] test_captcha FAILED: {}", e);
            Err(e)
        }
    }
}

/// 批量验证码识别测试。
///
/// 连续执行 count 次验证码识别，若某次失败则中断并返回已完成的结果。
#[tauri::command]
pub async fn batch_test_captcha(
    state: State<'_, AppState>,
    mode: String,
    count: u32,
) -> Result<Vec<CaptchaTestResultFrontend>, String> {
    tracing::info!(
        "[Captcha] batch_test_captcha called, mode={}, count={}",
        mode,
        count
    );
    let state_ref = &*state;
    let mut results = Vec::new();
    if mode == "manual" {
        return Err("手动模式不支持批量测试，请使用单次测试".to_string());
    }
    for i in 0..count {
        match do_test_captcha(Some(state_ref), &mode, None).await {
            Ok(mut result) => {
                result.id = i + 1;
                results.push(result);
            }
            Err(e) => {
                tracing::error!(
                    "[Captcha] batch_test_captcha: test #{} failed: {}",
                    i + 1,
                    e
                );
                break;
            }
        }
    }
    tracing::info!(
        "[Captcha] batch_test_captcha completed, results={}",
        results.len()
    );
    Ok(results)
}

/// 初始化本地 ONNX 推理后端，加载模型到内存。
///
/// 若已初始化则跳过，若模型文件缺失则返回错误。
#[tauri::command]
pub async fn init_local_ocr(state: State<'_, AppState>) -> Result<(), String> {
    tracing::info!("[Captcha] init_local_ocr called");

    let local_ocr = state.local_ocr.clone();
    {
        let guard = local_ocr
            .lock()
            .map_err(|e| format!("获取ONNX锁失败: {}", e))?;
        if guard.is_some() {
            tracing::info!("[Captcha] init_local_ocr: already initialized, skipping");
            return Ok(());
        }
    }

    let config = state.config.read().await;
    let model_path = config.onnx_model_path();
    let missing = CasOnnxBackend::missing_model_files(&model_path);
    if !missing.is_empty() {
        let missing_str = missing.join(", ");
        tracing::error!(
            "[Captcha] init_local_ocr: 模型文件不完整，缺少: {}",
            missing_str
        );
        return Err(format!("模型文件不完整，缺少: {}", missing_str));
    }

    tracing::info!(
        "[Captcha] init_local_ocr: loading models from {:?}",
        model_path
    );
    let backend = tokio::task::spawn_blocking(move || {
        CasOnnxBackend::load(&model_path).map_err(|e| format!("加载ONNX模型失败: {}", e))
    })
    .await
    .map_err(|e| format!("ONNX加载任务执行失败: {}", e))??;

    let mut guard = local_ocr
        .lock()
        .map_err(|e| format!("获取ONNX锁失败: {}", e))?;
    *guard = Some(backend);
    tracing::info!("[Captcha] init_local_ocr: ONNX models loaded successfully");
    Ok(())
}

/// 卸载本地 ONNX 推理后端，释放模型占用的内存。
#[tauri::command]
pub async fn unload_local_ocr(state: State<'_, AppState>) -> Result<(), String> {
    tracing::info!("[Captcha] unload_local_ocr called");

    let local_ocr = state.local_ocr.clone();
    let mut guard = local_ocr
        .lock()
        .map_err(|e| format!("获取ONNX锁失败: {}", e))?;
    if guard.is_some() {
        *guard = None;
        tracing::info!("[Captcha] unload_local_ocr: ONNX models unloaded");
    } else {
        tracing::info!("[Captcha] unload_local_ocr: ONNX not loaded, nothing to unload");
    }
    Ok(())
}

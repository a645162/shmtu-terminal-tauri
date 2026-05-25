use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use shmtu_cas::captcha::CaptchaResolver;
use tauri::State;

use crate::config::CaptchaMode;
use crate::state::AppState;

/// 验证码测试结果（前端展示用）
#[derive(Debug, Clone, Serialize)]
pub struct CaptchaTestResultFrontend {
    pub id: u32,
    pub success: bool,
    pub expression: String,
    pub answer: String,
    pub duration_ms: u64,
    pub mode: String,
    pub error: Option<String>,
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
pub async fn get_captcha_with_execution() -> Result<CaptchaChallengeResponse, String> {
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
                    Ok(CaptchaChallengeResponse {
                        captcha_image: base64_image,
                        execution: challenge.execution,
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
) -> Result<CaptchaTestResultFrontend, String> {
    tracing::debug!("[Captcha] do_test_captcha called, mode={}", mode);

    let captcha_mode = match mode {
        "manual" => CaptchaMode::Manual,
        "remote_ocr" => CaptchaMode::RemoteOcr,
        "local_onnx" => CaptchaMode::LocalOnnx,
        _ => {
            tracing::error!("[Captcha] do_test_captcha: unknown mode: {}", mode);
            return Err(format!("未知验证码模式: {}", mode));
        }
    };

    let start = std::time::Instant::now();

    use shmtu_cas::cas::epay::EpayAuth;
    let mut epay = EpayAuth::new().map_err(|e| {
        tracing::error!("[Captcha] do_test_captcha: EpayAuth::new() failed: {}", e);
        format!("创建EpayAuth失败: {}", e)
    })?;

    let _ = epay.probe_login().await.map_err(|e| {
        tracing::error!("[Captcha] do_test_captcha: probe_login failed: {}", e);
        format!("探测登录状态失败: {}", e)
    })?;

    let challenge = epay.prepare_challenge().await.map_err(|e| {
        tracing::error!("[Captcha] do_test_captcha: prepare_challenge failed: {}", e);
        format!("获取验证码失败: {}", e)
    })?;

    let duration_ms = start.elapsed().as_millis() as u64;

    let (expression, answer, success, error) = match captcha_mode {
        CaptchaMode::Manual => {
            tracing::info!("[Captcha] do_test_captcha: manual mode, no recognition");
            (
                String::new(),
                String::new(),
                false,
                Some("手动模式需要用户输入".to_string()),
            )
        }
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
                tracing::error!("[Captcha] do_test_captcha: remote OCR not configured");
                (
                    String::new(),
                    String::new(),
                    false,
                    Some("未配置远程OCR服务器地址".to_string()),
                )
            } else {
                tracing::info!(
                    "[Captcha] do_test_captcha: using remote OCR {}:{}",
                    host,
                    port
                );
                let resolver = shmtu_cas::captcha::OcrCaptchaResolver::new(&host, port)
                    .with_retries(retry_count);
                match resolver.resolve(&challenge.captcha_image).await {
                    Ok(result) => {
                        // OCR 返回表达式（如 "3+5="）和计算后的答案
                        let expr = result.value.clone();
                        let ans = result.into_final_answer();
                        tracing::info!("[Captcha] do_test_captcha: OCR success, answer={}", ans);
                        (expr, ans, true, None)
                    }
                    Err(e) => {
                        tracing::error!("[Captcha] do_test_captcha: OCR failed: {}", e);
                        (
                            String::new(),
                            String::new(),
                            false,
                            Some(format!("远程OCR识别失败: {}", e)),
                        )
                    }
                }
            }
        }
        CaptchaMode::LocalOnnx => {
            tracing::error!("[Captcha] do_test_captcha: local ONNX not implemented");
            (
                String::new(),
                String::new(),
                false,
                Some("本地ONNX模式暂未实现，请使用远程OCR或手动模式".to_string()),
            )
        }
    };

    Ok(CaptchaTestResultFrontend {
        id: 1,
        success,
        expression,
        answer,
        duration_ms,
        mode: mode.to_string(),
        error,
    })
}

/// 单次验证码识别测试。
///
/// 获取验证码图片后尝试用指定模式识别，返回识别结果和耗时。
#[tauri::command]
pub async fn test_captcha(
    state: State<'_, AppState>,
    mode: String,
) -> Result<CaptchaTestResultFrontend, String> {
    tracing::info!("[Captcha] test_captcha called, mode={}", mode);
    match do_test_captcha(Some(&state), &mode).await {
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
    for i in 0..count {
        match do_test_captcha(Some(state_ref), &mode).await {
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

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use shmtu_cas::captcha::CaptchaResolver;
use tauri::State;

use crate::config::CaptchaMode;
use crate::state::AppState;

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

#[derive(Debug, Clone, Serialize)]
pub struct CaptchaChallengeResponse {
    pub captcha_image: String,
    pub execution: String,
}

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

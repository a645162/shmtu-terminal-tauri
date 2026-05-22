use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use shmtu_cas::captcha::CaptchaResolver;
use tauri::State;

use crate::config::CaptchaMode;
use crate::state::AppState;

/// 验证码测试结果（与前端 CaptchaTestResult 对齐）
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
    use shmtu_cas::cas::epay::EpayAuth;

    let mut epay = EpayAuth::new().map_err(|e| format!("创建EpayAuth失败: {}", e))?;

    // 探测是否需要登录
    match epay.probe_login().await.map_err(|e| format!("探测登录状态失败: {}", e))? {
        shmtu_cas::cas::epay::LoginProbe::AlreadyLoggedIn => {
            return Err("已登录，无需验证码".to_string());
        }
        shmtu_cas::cas::epay::LoginProbe::NeedLogin { .. } => {}
    }

    // 获取验证码
    let challenge = epay
        .prepare_challenge()
        .await
        .map_err(|e| format!("获取验证码失败: {}", e))?;

    // 将验证码图片编码为 Base64
    let base64_image = BASE64.encode(&challenge.captcha_image);

    Ok(format!("data:image/png;base64,{}", base64_image))
}

#[tauri::command]
pub async fn test_captcha(
    state: State<'_, AppState>,
    mode: String,
) -> Result<CaptchaTestResultFrontend, String> {
    let captcha_mode = match mode.as_str() {
        "manual" => CaptchaMode::Manual,
        "remote_ocr" => CaptchaMode::RemoteOcr,
        "local_onnx" => CaptchaMode::LocalOnnx,
        _ => return Err(format!("未知验证码模式: {}", mode)),
    };

    let start = std::time::Instant::now();

    // 获取验证码
    use shmtu_cas::cas::epay::EpayAuth;
    let mut epay = EpayAuth::new().map_err(|e| format!("创建EpayAuth失败: {}", e))?;
    let _ = epay.probe_login().await.map_err(|e| format!("探测登录状态失败: {}", e))?;
    let challenge = epay
        .prepare_challenge()
        .await
        .map_err(|e| format!("获取验证码失败: {}", e))?;

    let duration_ms = start.elapsed().as_millis() as u64;

    // 根据模式识别验证码
    let (expression, answer, success, error) = match captcha_mode {
        CaptchaMode::Manual => {
            // 手动模式：只返回图片，不进行识别
            (
                String::new(),
                String::new(),
                false,
                Some("手动模式需要用户输入".to_string()),
            )
        }
        CaptchaMode::RemoteOcr => {
            let config = state.config.read().await;
            let captcha_config = &config.get().captcha;
            let host = &captcha_config.remote_ocr_host;
            let port = captcha_config.remote_ocr_port;

            if host.is_empty() || port == 0 {
                (
                    String::new(),
                    String::new(),
                    false,
                    Some("未配置远程OCR服务器地址".to_string()),
                )
            } else {
                // 使用 shmtu-cas-rs 的远程 OCR 功能
                let resolver = shmtu_cas::captcha::OcrCaptchaResolver::new(host, port)
                    .with_retries(captcha_config.ocr_retry_count);
                match resolver.resolve(&challenge.captcha_image).await {
                    Ok(result) => {
                        let expr = result.value.clone();
                        let ans = result.into_final_answer();
                        (expr, ans, true, None)
                    }
                    Err(e) => (
                        String::new(),
                        String::new(),
                        false,
                        Some(format!("远程OCR识别失败: {}", e)),
                    ),
                }
            }
        }
        CaptchaMode::LocalOnnx => {
            // 本地 ONNX 模式暂未实现
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
        mode,
        error,
    })
}

#[tauri::command]
pub async fn batch_test_captcha(
    state: State<'_, AppState>,
    mode: String,
    count: u32,
) -> Result<Vec<CaptchaTestResultFrontend>, String> {
    let mut results = Vec::new();
    for i in 0..count {
        let mut result = test_captcha(state.clone(), mode.clone()).await?;
        result.id = i + 1;
        results.push(result);
    }
    Ok(results)
}
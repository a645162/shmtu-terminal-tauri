use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use shmtu_cas::captcha::CaptchaResolver;
use shmtu_ocr::backend::CasOnnxBackend;
use shmtu_ocr::const_value;
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::config::CaptchaMode;
use crate::state::{AppState, CaptchaTestSession};

const LOCAL_OCR_MODEL_DOWNLOAD_EVENT: &str = "local-ocr-model-download";

#[derive(Debug, Clone, Serialize)]
pub struct LocalOcrModelStatus {
    pub model_dir: String,
    pub ready: bool,
    pub total_files: u32,
    pub existing_files: u32,
    pub missing_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalOcrModelDownloadProgress {
    pub phase: String,
    pub model_dir: String,
    pub total_files: u32,
    pub completed_files: u32,
    pub current_file_index: Option<u32>,
    pub current_file_name: Option<String>,
    pub current_file_progress: f32,
    pub overall_progress: f32,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub message: String,
}

fn local_ocr_model_status(model_path: &Path) -> LocalOcrModelStatus {
    let total_files = 3_u32;
    let missing_files = CasOnnxBackend::missing_model_files(model_path)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let existing_files = total_files.saturating_sub(missing_files.len() as u32);
    LocalOcrModelStatus {
        model_dir: model_path.display().to_string(),
        ready: missing_files.is_empty(),
        total_files,
        existing_files,
        missing_files,
    }
}

fn emit_local_ocr_download_progress(app: &AppHandle, progress: &LocalOcrModelDownloadProgress) {
    let _ = app.emit(LOCAL_OCR_MODEL_DOWNLOAD_EVENT, progress);
}

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

/// 计算文件的 SHA256 哈希值（异步，用于校验模型完整性）。
async fn compute_file_sha256(path: &Path) -> Result<String, String> {
    use tokio::io::AsyncReadExt;
    let mut hasher = Sha256::new();
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("打开文件失败: {}", e))?;
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).await.map_err(|e| format!("读取文件失败: {}", e))?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// 解析 SHA256SUMS.txt 格式的校验文件。
fn parse_checksum_file(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.splitn(2, "  ").collect();
        if parts.len() == 2 {
            let hash = parts[0].trim();
            let name = parts[1].trim().trim_start_matches('*');
            if hash.len() == 64 {
                map.insert(name.to_string(), hash.to_string());
            }
        }
    }
    map
}

async fn ensure_local_ocr_model_files(
    app: &AppHandle,
    state: &AppState,
) -> Result<LocalOcrModelStatus, String> {
    let _download_guard = state.local_ocr_download_lock.lock().await;
    let config = state.config.read().await;
    let model_path = config.onnx_model_path();
    drop(config);

    let status = local_ocr_model_status(&model_path);
    if status.ready {
        emit_local_ocr_download_progress(
            app,
            &LocalOcrModelDownloadProgress {
                phase: "completed".to_string(),
                model_dir: status.model_dir.clone(),
                total_files: status.total_files,
                completed_files: status.total_files,
                current_file_index: None,
                current_file_name: None,
                current_file_progress: 1.0,
                overall_progress: 1.0,
                downloaded_bytes: None,
                total_bytes: None,
                message: "本地 OCR 模型已就绪".to_string(),
            },
        );
        return Ok(status);
    }

    state
        .local_ocr_download_cancel
        .store(false, Ordering::SeqCst);
    state
        .local_ocr_download_active
        .store(true, Ordering::SeqCst);

    let result = async {
        tokio::fs::create_dir_all(&model_path)
            .await
            .map_err(|e| format!("创建模型目录失败: {}", e))?;

        let files = [
            const_value::MODEL_ONNX_EQUAL_FP32,
            const_value::MODEL_ONNX_OPERATOR_FP32,
            const_value::MODEL_ONNX_DIGIT_FP32,
        ];
        let total_files = files.len() as u32;
        let client = reqwest::Client::new();

        // 获取校验文件
        let checksums = match client
            .get(const_value::MODEL_ONNX_CHECKSUM_URL)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let text = resp.text().await.unwrap_or_default();
                parse_checksum_file(&text)
            }
            Ok(resp) => {
                tracing::warn!("获取校验文件失败 (HTTP {})，跳过完整性验证", resp.status());
                std::collections::HashMap::new()
            }
            Err(e) => {
                tracing::warn!("获取校验文件失败 ({})，跳过完整性验证", e);
                std::collections::HashMap::new()
            }
        };

        let mut completed_files = 0_u32;

        emit_local_ocr_download_progress(
            app,
            &LocalOcrModelDownloadProgress {
                phase: "checking".to_string(),
                model_dir: model_path.display().to_string(),
                total_files,
                completed_files: 0,
                current_file_index: None,
                current_file_name: None,
                current_file_progress: 0.0,
                overall_progress: status.existing_files as f32 / total_files as f32,
                downloaded_bytes: None,
                total_bytes: None,
                message: "正在检查本地 OCR 模型...".to_string(),
            },
        );

        for (index, file_name) in files.iter().enumerate() {
            if state.local_ocr_download_cancel.load(Ordering::SeqCst) {
                return Err("本地 OCR 模型下载已取消".to_string());
            }

            let dest_path = model_path.join(file_name);
            if dest_path.exists() {
                // 已存在的文件也校验完整性
                let existing_ok = if let Some(expected) = checksums.get(*file_name) {
                    match compute_file_sha256(&dest_path).await {
                        Ok(actual) => actual == *expected,
                        Err(_) => false,
                    }
                } else {
                    true
                };
                if existing_ok {
                    completed_files += 1;
                    emit_local_ocr_download_progress(
                        app,
                        &LocalOcrModelDownloadProgress {
                            phase: "downloading".to_string(),
                            model_dir: model_path.display().to_string(),
                            total_files,
                            completed_files,
                            current_file_index: Some(index as u32 + 1),
                            current_file_name: Some((*file_name).to_string()),
                            current_file_progress: 1.0,
                            overall_progress: completed_files as f32 / total_files as f32,
                            downloaded_bytes: None,
                            total_bytes: None,
                            message: format!("模型已存在，跳过 {}", file_name),
                        },
                    );
                    continue;
                }
                // 校验失败，删除损坏文件后重新下载
                tracing::warn!("{} 已存在但校验失败，删除后重新下载", file_name);
                let _ = tokio::fs::remove_file(&dest_path).await;
            }

            let max_attempts = 3_u32;
            let mut file_download_ok = false;

            for attempt in 1..=max_attempts {
                if state.local_ocr_download_cancel.load(Ordering::SeqCst) {
                    return Err("本地 OCR 模型下载已取消".to_string());
                }

                let url = format!("{}/{}", const_value::MODEL_ONNX_BASE_URL, file_name);
                let tmp_path = model_path.join(format!("{}.download", file_name));
                if tmp_path.exists() {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                }

                let attempt_msg = if attempt > 1 {
                    format!(
                        "校验失败，第 {}/{} 次重试下载 {}/{}: {}",
                        attempt, max_attempts, index + 1, total_files, file_name
                    )
                } else {
                    format!("正在下载模型 {}/{}: {}", index + 1, total_files, file_name)
                };

                emit_local_ocr_download_progress(
                    app,
                    &LocalOcrModelDownloadProgress {
                        phase: "downloading".to_string(),
                        model_dir: model_path.display().to_string(),
                        total_files,
                        completed_files,
                        current_file_index: Some(index as u32 + 1),
                        current_file_name: Some((*file_name).to_string()),
                        current_file_progress: 0.0,
                        overall_progress: completed_files as f32 / total_files as f32,
                        downloaded_bytes: Some(0),
                        total_bytes: None,
                        message: attempt_msg,
                    },
                );

                let mut response = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| format!("下载模型失败: {}", e))?
                    .error_for_status()
                    .map_err(|e| format!("模型接口返回异常状态: {}", e))?;
                let total_bytes = response.content_length();
                let mut output = tokio::fs::File::create(&tmp_path)
                    .await
                    .map_err(|e| format!("创建模型文件失败: {}", e))?;
                let mut downloaded = 0_u64;

                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|e| format!("读取模型下载流失败: {}", e))?
                {
                    if state.local_ocr_download_cancel.load(Ordering::SeqCst) {
                        let _ = output.flush().await;
                        drop(output);
                        let _ = tokio::fs::remove_file(&tmp_path).await;
                        emit_local_ocr_download_progress(
                            app,
                            &LocalOcrModelDownloadProgress {
                                phase: "cancelled".to_string(),
                                model_dir: model_path.display().to_string(),
                                total_files,
                                completed_files,
                                current_file_index: Some(index as u32 + 1),
                                current_file_name: Some((*file_name).to_string()),
                                current_file_progress: if let Some(total) = total_bytes {
                                    if total > 0 {
                                        downloaded as f32 / total as f32
                                    } else {
                                        0.0
                                    }
                                } else {
                                    0.0
                                },
                                overall_progress: completed_files as f32 / total_files as f32,
                                downloaded_bytes: Some(downloaded),
                                total_bytes,
                                message: format!("已取消下载，未完成文件已删除: {}", file_name),
                            },
                        );
                        return Err("本地 OCR 模型下载已取消".to_string());
                    }

                    output
                        .write_all(&chunk)
                        .await
                        .map_err(|e| format!("写入模型文件失败: {}", e))?;
                    downloaded += chunk.len() as u64;

                    let current_file_progress = if let Some(total) = total_bytes {
                        if total > 0 {
                            downloaded as f32 / total as f32
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };
                    let overall_progress =
                        (completed_files as f32 + current_file_progress) / total_files as f32;

                    emit_local_ocr_download_progress(
                        app,
                        &LocalOcrModelDownloadProgress {
                            phase: "downloading".to_string(),
                            model_dir: model_path.display().to_string(),
                            total_files,
                            completed_files,
                            current_file_index: Some(index as u32 + 1),
                            current_file_name: Some((*file_name).to_string()),
                            current_file_progress,
                            overall_progress,
                            downloaded_bytes: Some(downloaded),
                            total_bytes,
                            message: format!(
                                "正在下载模型 {}/{}: {}",
                                index + 1,
                                total_files,
                                file_name
                            ),
                        },
                    );
                }

                output
                    .flush()
                    .await
                    .map_err(|e| format!("刷新模型文件失败: {}", e))?;
                drop(output);
                tokio::fs::rename(&tmp_path, &dest_path)
                    .await
                    .map_err(|e| format!("保存模型文件失败: {}", e))?;

                // 校验 SHA256
                if let Some(expected) = checksums.get(*file_name) {
                    match compute_file_sha256(&dest_path).await {
                        Ok(actual) if actual == *expected => {
                            file_download_ok = true;
                        }
                        Ok(actual) => {
                            tracing::warn!(
                                "{} 校验失败 (期望: {}，实际: {})",
                                file_name, expected, actual
                            );
                            let _ = tokio::fs::remove_file(&dest_path).await;
                            if attempt == max_attempts {
                                return Err(format!(
                                    "{} 校验失败，已重试 {} 次仍不通过",
                                    file_name, max_attempts
                                ));
                            }
                            continue; // 重试
                        }
                        Err(e) => {
                            tracing::warn!("{} 校验读取失败: {}", file_name, e);
                            let _ = tokio::fs::remove_file(&dest_path).await;
                            if attempt == max_attempts {
                                return Err(e);
                            }
                            continue; // 重试
                        }
                    }
                } else {
                    file_download_ok = true;
                }

                break; // 下载成功
            }

            if !file_download_ok {
                return Err(format!("{} 下载失败", file_name));
            }

            completed_files += 1;
            emit_local_ocr_download_progress(
                app,
                &LocalOcrModelDownloadProgress {
                    phase: "downloading".to_string(),
                    model_dir: model_path.display().to_string(),
                    total_files,
                    completed_files,
                    current_file_index: Some(index as u32 + 1),
                    current_file_name: Some((*file_name).to_string()),
                    current_file_progress: 1.0,
                    overall_progress: completed_files as f32 / total_files as f32,
                    downloaded_bytes: None,
                    total_bytes: None,
                    message: format!("模型下载完成: {}", file_name),
                },
            );
        }

        let final_status = local_ocr_model_status(&model_path);
        emit_local_ocr_download_progress(
            app,
            &LocalOcrModelDownloadProgress {
                phase: "completed".to_string(),
                model_dir: final_status.model_dir.clone(),
                total_files: final_status.total_files,
                completed_files: final_status.total_files,
                current_file_index: None,
                current_file_name: None,
                current_file_progress: 1.0,
                overall_progress: 1.0,
                downloaded_bytes: None,
                total_bytes: None,
                message: "本地 OCR 模型下载完成".to_string(),
            },
        );
        Ok(final_status)
    }
    .await;

    state
        .local_ocr_download_active
        .store(false, Ordering::SeqCst);
    state
        .local_ocr_download_cancel
        .store(false, Ordering::SeqCst);

    if let Err(error) = &result {
        if !error.contains("已取消") {
            emit_local_ocr_download_progress(
                app,
                &LocalOcrModelDownloadProgress {
                    phase: "error".to_string(),
                    model_dir: model_path.display().to_string(),
                    total_files: status.total_files,
                    completed_files: 0,
                    current_file_index: None,
                    current_file_name: None,
                    current_file_progress: 0.0,
                    overall_progress: 0.0,
                    downloaded_bytes: None,
                    total_bytes: None,
                    message: error.clone(),
                },
            );
        }
    }

    result
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

#[tauri::command]
pub async fn get_local_ocr_model_status(
    state: State<'_, AppState>,
) -> Result<LocalOcrModelStatus, String> {
    let config = state.config.read().await;
    Ok(local_ocr_model_status(&config.onnx_model_path()))
}

#[tauri::command]
pub async fn ensure_local_ocr_models(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LocalOcrModelStatus, String> {
    tracing::info!("[Captcha] ensure_local_ocr_models called");
    ensure_local_ocr_model_files(&app, &state).await
}

#[tauri::command]
pub async fn cancel_local_ocr_model_download(state: State<'_, AppState>) -> Result<(), String> {
    tracing::info!("[Captcha] cancel_local_ocr_model_download called");
    if !state.local_ocr_download_active.load(Ordering::SeqCst) {
        return Ok(());
    }
    state
        .local_ocr_download_cancel
        .store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn delete_local_ocr_models(state: State<'_, AppState>) -> Result<LocalOcrModelStatus, String> {
    tracing::info!("[Captcha] delete_local_ocr_models called");

    let _download_guard = state.local_ocr_download_lock.lock().await;
    state
        .local_ocr_download_cancel
        .store(true, Ordering::SeqCst);
    state
        .local_ocr_download_active
        .store(false, Ordering::SeqCst);

    {
        let mut guard = state
            .local_ocr
            .lock()
            .map_err(|e| format!("获取ONNX锁失败: {}", e))?;
        *guard = None;
    }

    let config = state.config.read().await;
    let model_path = config.onnx_model_path();
    drop(config);

    let files = [
        const_value::MODEL_ONNX_EQUAL_FP32,
        const_value::MODEL_ONNX_OPERATOR_FP32,
        const_value::MODEL_ONNX_DIGIT_FP32,
    ];

    for file_name in files {
        let file_path = model_path.join(file_name);
        if file_path.exists() {
            tokio::fs::remove_file(&file_path)
                .await
                .map_err(|e| format!("删除模型文件失败 {}: {}", file_name, e))?;
        }

        let partial_path = model_path.join(format!("{}.download", file_name));
        if partial_path.exists() {
            tokio::fs::remove_file(&partial_path)
                .await
                .map_err(|e| format!("删除临时模型文件失败 {}: {}", file_name, e))?;
        }
    }

    Ok(local_ocr_model_status(&model_path))
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

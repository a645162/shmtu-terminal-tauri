//! Tauri commands for 一卡通个人账户详情（CAS /epay/personaccount/index）
//!
//! 登录流程复用账单同步那套: 所有 CaptchaMode 均由 sync 模块统一处理,
//! 包括 Manual (手动输入) / RemoteOcr (远程 OCR) / RemoteOcrHttp (RESTful OCR) / LocalOnnx (本地 ONNX 模型)。
//!
//! 这里只负责 **用已有 cookies 拉取 + 解析 + 缓存**。
//! cookies 过期时返回 SESSION_EXPIRED 错误, 前端 UI 提示用户"请先同步账单/重新登录"。

use shmtu_cas::cas::epay::{EpayAuth, LoginProbe};
use shmtu_cas::parser::person_account::parse_person_account;
use tauri::State;

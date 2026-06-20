# SHMTU Terminal Tauri — OCR (ONNX) 架构指引

本文档只描述 Tauri 桌面端当前实际使用的 OCR 方案：`shmtu-ocr` crate 提供的 ONNX Runtime 后端。  
Tauri 与 C# 桌面端共享同一类 ONNX 模型命名、版本语义和下载来源；Android 端虽有历史 NCNN 实现，但不属于这里的 Tauri 本地推理链路。

## 架构总览

```
Tauri Frontend
    │
    ├─ 验证码测试 / 同步设置 / 本地模型下载
    ▼
Tauri Commands
    │
    ├─ commands/captcha.rs
    │   ├─ 管理模型版本 / tag / backbone / precision
    │   ├─ 下载 ONNX 模型
    │   └─ 初始化 / 卸载本地 OCR
    │
    └─ ocr_server/mod.rs
        └─ 通过 HTTP 暴露本机 `local_ocr`
             给 Android / Tauri / C# 等客户端复用
    ▼
AppState.local_ocr: Arc<Mutex<Option<OcrBackend>>>
    ▼
shmtu-ocr::backend::OcrBackend
    ├─ V1Backend: 3 个 ResNet ONNX
    └─ V2Backend: 1 个 MobileNetV3 ONNX
```

## 当前代码事实

- Tauri 本地 OCR 入口是 `shmtu_ocr::backend::OcrBackend`。
- `OcrBackend::load(version, dir)` 按 `ModelVersion::V1` / `V2` 选择 ONNX 实现。
- `ocr_server/mod.rs` 暴露的是同一个 `local_ocr` 实例，不存在额外的 Tauri-NCNN 推理通道。
- 下载 v2 模型时，`download_v2()` 在 manifest 中固定查找 `engine="onnx"` 的 artifact。

## 模型版本

### v1

- 三个独立 ONNX 文件：
  - `resnet18_equal_symbol_latest.onnx`
  - `resnet18_operator_latest.onnx`
  - `resnet34_digit_latest.onnx`
- 兼容老用户配置。

### v2

- 单个 ONNX 文件。
- 文件名格式：

```text
<backbone>.trislot_decoder.v2_0.<precision>.onnx
```

- 默认配置来自 `shmtu_ocr::const_value::v2`：
  - `DEFAULT_TAG = "v2.0.5"`
  - `DEFAULT_BACKBONE = "mobilenet_v3_small"`
  - `DEFAULT_PRECISION = "fp16"`

## 模型下载链路

Tauri 端下载逻辑位于 `src-tauri/src/commands/captcha.rs`，核心依赖 `shmtu_ocr::downloader::download_v2()`。

下载流程：

1. 读取配置中的 `model_tag`、`model_backbone`、`model_precision`
2. 解析目标 release tag
3. 拉取 `model-assets.json`
4. 按 `family=trislot_decoder`、`backbone`、`precision` 选择 `onnx` artifact
5. 下载到本地模型目录并做 SHA256 校验

v2 本地模型目录默认来自 `config.onnx_model_path()`。

## 本地 OCR 生命周期

### 初始化

- 前端通过 `init_local_ocr` 或首次 OCR 请求触发加载。
- 后端根据 `config.captcha.model_version` 计算模型目录。
- `OcrBackend::missing_model_files()` 先校验文件是否齐全。
- `OcrBackend::load()` 在阻塞线程中加载 ONNX Runtime session。

### 复用

- 已加载的 backend 保存在 `AppState.local_ocr`。
- `ocr_server/mod.rs` 的 `POST /api/ocr` 会复用该实例。
- 前端本地测试和 HTTP 服务共用同一套 ONNX backend。

### 卸载

- `unload_local_ocr` 把 `local_ocr` 置空。
- 再次请求时按需重新加载。

## OCR HTTP 服务

Tauri 端 HTTP OCR 服务用于把桌面端本地 ONNX 推理共享给其他客户端。

- 请求体：`{"imageBase64":"..."}`
- 响应体：`{"success":true,"expression":"3+5=8","result":8}`
- 懒加载：首次 `POST /api/ocr` 才真正加载模型
- 状态端点：`/api/health`、`/api/status`

这条链路对应文件：

- `src-tauri/src/ocr_server/mod.rs`
- `src-tauri/src/commands/ocr_server.rs`
- `src-tauri/src/state.rs`

## 与 C# 侧的关系

- 两边都使用 ONNX 模型，而不是 NCNN。
- 公共语义保持一致：模型版本、backbone、precision、文件命名规则。
- Tauri 侧 `const_value` 注释已经明确“对齐 C# 的 ConstValue”。

## 关键文件

- `src-tauri/src/commands/captcha.rs`
- `src-tauri/src/ocr_server/mod.rs`
- `src-tauri/src/state.rs`
- `src-tauri/vendor/shmtu-cas-rs/ocr/shmtu-ocr/src/backend/mod.rs`
- `src-tauri/vendor/shmtu-cas-rs/ocr/shmtu-ocr/src/backend/v1.rs`
- `src-tauri/vendor/shmtu-cas-rs/ocr/shmtu-ocr/src/backend/v2.rs`
- `src-tauri/vendor/shmtu-cas-rs/ocr/shmtu-ocr/src/downloader.rs`

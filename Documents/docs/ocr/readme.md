# Tauri 端 OCR (ONNX) 架构说明

本文档描述 Tauri 桌面端当前实际落地的 OCR 方案。结论很简单：Tauri 本地 OCR 走 ONNX Runtime，不走 NCNN。

## 背景

- Tauri 本地推理由 `vendor/shmtu-cas-rs/ocr/shmtu-ocr/` 提供。
- 对外统一入口是 `shmtu_ocr::backend::OcrBackend`。
- `OcrBackend` 内部分为：
  - `V1Backend`：3 个 ONNX 模型
  - `V2Backend`：1 个 ONNX 模型

## 架构概要

- 前端设置页和验证码测试页通过 Tauri command 管理本地模型。
- `commands/captcha.rs` 负责模型下载、状态查询、初始化和卸载。
- `ocr_server/mod.rs` 把 `AppState.local_ocr` 暴露成 HTTP OCR 服务。
- `local_ocr` 持有的是 `Option<OcrBackend>`，因此 HTTP OCR 和前端本地测试共用同一个 ONNX backend。

## 模型下载

v2 下载复用 `shmtu_ocr::downloader::download_v2()`：

1. 解析 tag
2. 拉取 `model-assets.json`
3. 根据 `backbone` 和 `precision` 选中目标模型
4. 固定选择 `engine="onnx"` 的 artifact
5. 下载并校验 SHA256

默认配置：

| 配置键 | 默认值 | 说明 |
|--------|--------|------|
| `ocr_v2_model_tag` | `""` | 为空时自动解析最新可用 tag |
| `ocr_v2_backbone` | `"mobilenet_v3_small"` | v2 ONNX 主干网络 |
| `ocr_v2_precision` | `"fp16"` | 文件命名和下载选择用 |

v2 文件名格式：

```text
<backbone>.trislot_decoder.v2_0.<precision>.onnx
```

## 本地加载与共享

- `init_local_ocr` 会按配置加载 v1 或 v2 ONNX 模型。
- `unload_local_ocr` 会释放当前 backend。
- `ocr_server/mod.rs` 首次收到 `/api/ocr` 请求时也会按需懒加载。
- 一旦加载完成，后续请求复用同一个 `OcrBackend`。

## 与 C# / Android 的关系

- C# 与 Tauri 在本地桌面推理上都使用 ONNX。
- Android 仓库里虽然保留过 NCNN 相关实现，但不应外推到 Tauri 侧架构。
- 文档和实现都应以 `OcrBackend` + ONNX Runtime 为准。

## 关键文件

- `src-tauri/src/commands/captcha.rs`
- `src-tauri/src/ocr_server/mod.rs`
- `src-tauri/src/state.rs`
- `src-tauri/vendor/shmtu-cas-rs/ocr/shmtu-ocr/src/backend/mod.rs`
- `src-tauri/vendor/shmtu-cas-rs/ocr/shmtu-ocr/src/backend/v1.rs`
- `src-tauri/vendor/shmtu-cas-rs/ocr/shmtu-ocr/src/backend/v2.rs`

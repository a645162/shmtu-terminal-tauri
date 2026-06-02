# 同步与验证码

## 同步的实际调用层次

前端发起同步后，链路大致是：

1. 页面或弹窗触发同步（增量 / 全量 / 单账号）
2. `appStore` 调用 `src/services/tauri.ts` 中对应方法
3. `tauri.ts` 通过 `invoke()` 调用 Tauri 命令
4. 进入 `src-tauri/src/commands/sync.rs` 对应命令
5. 命令层从 `State<AppState>` 获取 `sync_service`
6. 进入 `src-tauri/src/sync/mod.rs` 的 `BillSyncService`
7. 通过 `shmtu-cas` 库完成 CAS 登录、拉取账单、解析、入库

## 同步入口一览

当前前端和命令层区分了以下同步入口：

| 命令 | 方法 | 说明 |
|------|------|------|
| `incremental_sync` | `sync_identity()` | 身份级增量同步 |
| `full_sync` | `full_sync_identity()` | 身份级全量同步 |
| `incremental_sync_account` | `sync_single_account_by_id()` | 单账号增量同步 |
| `full_sync_account` | `full_sync_single_account()` | 单账号全量同步 |

这不是重复设计，而是把用户场景拆开了：

- 身份级同步：遍历该身份下所有启用的账号，逐个同步
- 单账号同步：只同步指定账号，适合排查单个账号问题
- 增量 vs 全量：增量只拉新数据并早停，全量先清除旧数据再重新拉取

## 增量同步 vs 全量同步的差异

### 增量同步

```
SyncOptions {
    start_page: 1,
    max_pages: 100,
    early_stop_threshold: 10,
    since_timestamp: 根据选择的范围计算,
}
```

- 默认最大翻 100 页
- 遇到连续 10 页旧数据时提前停止
- 保留已有数据，只追加新数据
- 适合日常更新

### 全量同步

```
SyncOptions {
    start_page: 1,
    max_pages: 1000,
    early_stop_threshold: u32::MAX,
    since_timestamp: 根据选择的范围计算,
}
```

- 默认最大翻 1000 页
- 不设早停阈值，尽量拉完
- **先清除**该身份下的旧合并数据和原始数据，再重新拉取
- 同时清除所有账号的旧 session，强制重新登录
- 适合首次使用或需要重新补数据

## 同步服务职责

`BillSyncService` 负责：

- 找到可同步账号（过滤已禁用、已毕业的账号）
- 根据配置决定验证码模式
- 处理手动验证码续传（`PendingManualSync` 机制）
- 执行多账号同步（按顺序逐个同步）
- 管理同步锁（`sync_lock: Mutex`，同一时间只允许一个同步任务）
- 汇总进度与结果

### 同步锁

`BillSyncService` 内部维护 `sync_lock: Mutex<()>`，所有同步入口（增量、全量、单账号）在执行前都要获取这个锁。这意味着：

- 同一时间只能有一个同步任务在运行
- 如果用户在同步进行中又点了同步，后一个请求会等待前一个完成
- 手动验证码续传（`sync_with_captcha`）也需要获取锁，确保验证码提交时没有其他同步在干扰

## 验证码模式

当前配置层支持四种验证码模式：

| 模式 | 枚举值 | 识别方式 | 依赖 |
|------|--------|---------|------|
| 手动 | `Manual` | 用户自己输入验证码图片文字 | 无 |
| 远程 TCP OCR | `RemoteOcr` | TCP 连接远程 OCR 服务 | 需配置 host + port |
| 远程 HTTP OCR | `RemoteOcrHttp` | HTTP 请求远程 OCR 服务 | 需配置 URL |
| 本地 ONNX | `LocalOnnx` | 本机加载 ONNX 模型推理 | 需下载模型 |

程序不是把 OCR 写死在同步逻辑里，而是根据配置动态选路径。同步服务通过 `ConfigAccess` 在运行时读取 `app_config.toml` 获取当前验证码模式。

### 自动登录流程（OCR 模式）

当验证码模式为 `RemoteOcr` 或 `RemoteOcrHttp` 时，同步服务走自动登录流程：

1. 创建 `EpayAuth` 实例
2. 探测登录状态（`probe_login`）
3. 如果需要登录，获取验证码 challenge
4. 调用对应 OCR 识别器识别验证码
5. 提交登录（用户名 + 密码 + 验证码 + execution）
6. 识别错误时重试（最多 `ocr_retry_count` 次，默认 5 次）
7. 密码错误直接返回失败，不重试

### 手动验证码流程

手动模式下的流程更复杂，因为需要在”等待用户输入”和”继续同步”之间切换：

1. 同步服务检测到需要登录
2. 获取验证码 challenge，将验证码图片 base64 编码
3. 存储 `PendingManualSync`（包含当前账号、剩余账号列表、同步选项等）
4. 返回 `MANUAL_CAPTCHA_REQUIRED` 错误给前端
5. 前端弹出验证码输入弹窗
6. 用户输入后，调用 `sync_with_captcha` 提交验证码
7. 如果验证码正确，继续同步当前账号
8. 当前账号完成后，检查是否还有剩余账号需要同步
9. 如果下一个账号也需要验证码，重复步骤 2-8
10. 所有账号完成后返回汇总结果

关键设计：`PendingManualSync` 保存了完整的同步上下文，包括剩余待同步的账号队列，这样用户只需输入一次验证码就能完成所有账号的同步。

### 会话复用

同步服务会优先尝试复用已保存的会话（`try_sync_with_saved_session`）：

1. 从数据库获取该账号的加密 session
2. 解密后恢复到 `EpayAuth` 中
3. 探测登录状态，如果仍然有效则直接跳过登录
4. 如果无效则走正常登录流程

会话保存在 `session_info` 表中，由 `SessionExpirationService` 定期检查有效性。

## 本地 OCR

本地 OCR 的额外复杂度在于它不仅是一个”识别模式”，还带了一套模型生命周期管理：

| 状态 | 说明 |
|------|------|
| 未下载 | 模型文件不存在，需要先下载 |
| 下载中 | 后台下载，可取消（`cancel_local_ocr_model_download`） |
| 已下载 | 模型文件完整，可加载 |
| 已加载 | `CasOnnxBackend` 实例在内存中，可直接推理 |
| 已卸载 | 主动释放内存（`unload_local_ocr`） |

相关命令：

- `get_local_ocr_model_status` — 查询模型状态
- `ensure_local_ocr_models` — 确保模型已下载（下载中则等待）
- `cancel_local_ocr_model_download` — 取消下载
- `delete_local_ocr_models` — 删除本地模型文件
- `init_local_ocr` — 加载模型到内存
- `unload_local_ocr` — 从内存卸载模型

并发保护：

- `local_ocr_download_lock`（`tokio::sync::Mutex`）串行化下载任务
- `local_ocr_download_cancel`（`AtomicBool`）标记取消
- `local_ocr_download_active`（`AtomicBool`）标记是否有下载正在进行
- `local_ocr` 使用 `std::sync::Mutex` 而非异步锁，因为 ONNX 推理是 CPU 密集操作

## 进度事件

同步进度通过 `SyncProgressCallback` 推送。`BillSyncService` 在每个阶段调用 `emit_progress`，将进度信息传递给命令层，命令层再通过 Tauri 事件系统推送到前端。

### SyncStatus 枚举

| 状态 | 含义 |
|------|------|
| `ProbingLogin` | 正在检查登录状态 |
| `GettingCaptcha` | 需要验证码（手动模式或刷新验证码） |
| `LoggingIn` | 已通过登录检查，准备拉取账单 |
| `Syncing { page, total }` | 正在拉取账单，当前第 page 页 / 共 total 页 |
| `Persisting` | 拉取完成，正在写入原始账单并合并 |
| `Completed` | 同步完成 |
| `Failed(String)` | 同步失败 |

### SyncProgress 结构

前端接收到的进度信息包含：

- `account_id` — 当前同步的账号 ID
- `current_account` — 当前账号名称
- `account_index` — 当前是第几个账号
- `total_accounts` — 总共需要同步的账号数
- `new_count` — 当前账号新增条数
- `pages_fetched` — 已拉取页数
- `total_new_count` — 累计新增条数
- `status` — 当前阶段

前端根据这些信息更新进度条和状态文字，让用户了解同步进展。

## 同步范围

`SyncRangePreset` 枚举定义了可选的时间范围：

| 枚举值 | 覆盖天数 | 对应 since_timestamp |
|--------|---------|---------------------|
| `Week` | 7 天 | now - 7 天 |
| `HalfMonth` | 15 天 | now - 15 天 |
| `Month` | 30 天 | now - 30 天 |
| `HalfYear` | 183 天 | now - 183 天 |
| `Year` | 365 天 | now - 365 天 |
| `All` | 不限 | None |

`since_timestamp` 传给 `shmtu_cas::sync::SyncOptions`，用于过滤只拉取该时间之后的账单。

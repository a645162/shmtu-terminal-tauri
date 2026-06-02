# 数据流与状态

## 前端状态中心

前端主要使用 Zustand 做状态管理，核心状态集中在 `src/stores/appStore.ts`。

### appStore 维护的状态

| 状态类别 | 内容 |
|---------|------|
| 身份 | 当前选中的身份、身份列表 |
| 账号 | 当前身份下的账号列表 |
| 账单 | 账单查询结果、分页信息、筛选条件 |
| 同步 | 同步进度、同步状态、是否正在同步 |
| 配置 | 当前加载的 `AppConfig` |
| 统计 | 各类统计结果缓存 |
| 弹窗 | 各弹窗开关状态（设置、统计、身份管理等） |

前端状态的核心特征：**统计结果是缓存式的**。统计不是从 store 持久化的，而是每次打开统计弹窗时重新从 Rust 端查询。这意味着统计始终反映的是数据库中的最新数据。

## 前端调用链

一条典型的前端到后端的调用链：

```
页面组件
  ↓ 触发操作（点击、选择等）
appStore（Zustand）
  ↓ 调用 action
src/services/tauri.ts
  ↓ 封装 invoke(...)
Tauri IPC
  ↓ 跨进程通信
Rust 命令层（commands/）
  ↓ 读取 State<AppState>
Rust 服务层
  ↓ 执行业务逻辑
数据库 / 网络
  ↓ 返回结果
原路返回 → appStore 更新 → UI 重渲染
```

具体示例——增量同步：

1. 用户在账单页点击”增量同步”
2. `appStore` 调用 `services/tauri.ts` 中的同步方法
3. `tauri.ts` 使用 `invoke('incremental_sync', { identityId, syncRange })`
4. Rust `commands/sync.rs` 中的 `incremental_sync` 接收参数
5. 从 `State<AppState>` 获取 `sync_service`
6. `BillSyncService` 执行登录、拉取、入库
7. 通过进度回调（`SyncProgressCallback`）推送进度
8. 前端监听进度事件更新 UI
9. 同步完成后 appStore 刷新账单列表

## Rust 全局状态

`src-tauri/src/state.rs` 中的 `AppState` 是所有 Rust 服务的容器，通过 `tauri::State<AppState>` 注入到命令中。

### AppState 持有的服务

| 字段 | 类型 | 用途 |
|------|------|------|
| `db_manager` | `Arc<RwLock<DatabaseManager>>` | 数据库连接与实体管理 |
| `crypto` | `Arc<RwLock<CryptoService>>` | 加密解密服务 |
| `config` | `Arc<RwLock<TomlConfig>>` | 配置管理 |
| `sync_service` | `Arc<RwLock<BillSyncService>>` | 账单同步服务 |
| `export_service` | `Arc<RwLock<ExportService>>` | 导入导出与快照 |
| `classifier` | `Arc<RwLock<Option<BillClassifier>>>` | 账单分类器（可选） |
| `db_file_manager` | `Arc<DatabaseFileManager>` | 规则文件管理（本地 + GitHub 远端） |
| `session_expiration_service` | `Arc<SessionExpirationService>` | 会话过期检查与续期 |
| `auto_sync_service` | `Arc<AutoSyncService>` | 自动同步定时服务 |
| `local_ocr` | `Arc<std::sync::Mutex<Option<CasOnnxBackend>>>` | 本地 ONNX 推理后端 |
| `local_ocr_download_cancel` | `Arc<AtomicBool>` | 模型下载取消标记 |
| `local_ocr_download_active` | `Arc<AtomicBool>` | 模型下载运行标记 |
| `local_ocr_download_lock` | `Arc<Mutex<()>>` | 模型下载串行化锁 |
| `captcha_test_session` | `Arc<Mutex<Option<CaptchaTestSession>>>` | 验证码测试会话 |

### 为什么这样组织

好处：

- **前端逻辑集中**：Zustand store 作为唯一状态源，避免分散的状态管理
- **Rust 服务生命周期统一**：所有服务由 `AppState` 统一初始化和持有，避免多处 new
- **命令层更薄**：命令只做参数转换和服务调用，不含业务逻辑
- **后续扩展功能时不容易散掉**：新增服务只需加入 `AppState`，新增命令只需调用已有服务

代价：

- `AppState` 会逐渐变大，需要定期审视是否有服务应该拆分
- 命令层和服务层的边界需要持续保持：命令层不写业务逻辑，服务层不感知 Tauri

### 并发安全设计

- `db_manager`、`config` 等高频访问服务使用 `RwLock`（读多写少场景）
- `local_ocr` 使用 `std::sync::Mutex`（CPU 密集的推理操作，避免异步锁带来的问题）
- `sync_service` 使用 `RwLock`，内部有 `sync_lock`（`tokio::sync::Mutex`）保证同一时间只有一个同步任务运行
- `local_ocr_download_lock` 串行化模型下载任务，防止并发下载

## 数据分层

当前至少存在以下几层数据：

```
┌──────────────────────────────────┐
│     统计结果缓存式前端状态        │  ← 每次查询重新计算，不持久化
├──────────────────────────────────┤
│     合并账单 (bill_merged)       │  ← 按身份聚合的展示层
├──────────────────────────────────┤
│     原始账单 (bill_original)     │  ← 按账号存储的原始数据
├──────────────────────────────────┤
│     身份与账号数据               │  ← 身份/账号/会话/操作日志
├──────────────────────────────────┤
│     配置数据 (app_config.toml)   │  ← TOML 配置文件
└──────────────────────────────────┘
```

对普通用户最容易混淆的是**原始账单**和**合并账单**的关系：

- **原始账单**：每个账号各自存储一份，是同步时从校园平台拉取的原始数据
- **合并账单**：按身份聚合，把该身份下所有账号的原始账单合并、去重后的展示层数据

程序中很多操作本质上是在这两层之间做整理：

| 操作 | 作用层级 | 说明 |
|------|---------|------|
| 增量同步 | 原始层 → 合并层 | 拉取新数据并自动合并 |
| 全量同步 | 原始层 → 合并层 | 清除后重新拉取并合并 |
| 去重 | 原始层 或 合并层 | 删除同一层中的重复记录 |
| 重建 | 原始层 → 合并层 | 从原始层重新生成合并层 |
| 分类 | 合并层 | 给合并账单打上分类标签 |

## 后台服务

程序启动时会自动启动两个后台服务：

### SessionExpirationService

- 定期检查各账号的会话是否即将过期
- 如果检测到即将过期，尝试自动续期
- 检查间隔由 `session.refresh_interval_minutes` 配置（默认 10 分钟）
- 可通过 `config.rs` 中的命令查询状态、手动触发检查、重启服务

### AutoSyncService

- 根据配置定时执行自动同步
- 间隔由 `sync.auto_sync_interval_minutes` 配置（默认 60 分钟）
- 同步范围由 `sync.auto_sync_range` 配置（默认 Month）
- 默认关闭，需在设置中启用 `auto_sync_enabled`

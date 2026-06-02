# 应用结构

海大终端当前是一个 `Vite + React + TypeScript + Tauri v2` 应用，整体分成三层。前端负责 UI 渲染和状态管理，Tauri 命令层作为桥接，Rust 服务层负责所有业务逻辑和数据持久化。

```
┌─────────────────────────────────────────┐
│             前端界面层 (React)            │
│  pages/ components/ stores/ services/    │
├─────────────────────────────────────────┤
│           Tauri 命令层 (commands/)       │
│  identity account bill sync captcha ...  │
├─────────────────────────────────────────┤
│          Rust 服务与数据层               │
│  state sync db config export crypto ... │
└─────────────────────────────────────────┘
```

## 1. 前端界面层

目录主要在：

- `src/pages/` — 各功能页面组件
- `src/components/` — 通用组件与图表组件
- `src/stores/` — Zustand 全局状态
- `src/services/tauri.ts` — 前端调用 Rust 命令的统一封装
- `src/hooks/` — 自定义 React Hooks
- `src/types/` — TypeScript 类型定义
- `src/utils/` — 工具函数（日期处理、翻译等）

页面组件结构：

| 页面目录 | 对应功能 |
|---------|---------|
| `pages/Home/` | 首页：统计卡片、趋势图、最近账单 |
| `pages/Bill/` | 账单页：查询、筛选、详情、同步入口 |
| `pages/Features/` | 功能大全：次级功能入口集合 |
| `pages/Settings/` | 设置弹窗：各类配置项 |
| `pages/IdentityManager/` | 身份与账号管理弹窗 |
| `pages/Statistics/` | 统计分析弹窗 |
| `pages/DataTransfer/` | 数据导入导出弹窗 |
| `pages/CaptchaTest/` | 验证码测试与 OCR 模型管理 |
| `pages/About/` | 关于页面 |

通用组件（`components/Common/`）包含弹窗、状态面板等跨页面复用组件：

| 组件 | 用途 |
|------|------|
| `AppProvider.tsx` | 全局 Provider，初始化应用状态 |
| `ManualCaptchaDialog.tsx` | 手动验证码输入弹窗 |
| `SyncRangeDialog.tsx` | 同步范围选择弹窗 |
| `BillDetailDialog.tsx` | 账单详情弹窗 |
| `SyncStatusPanel.tsx` | 同步状态面板 |
| `StartupPasswordDialog.tsx` | 启动密码输入弹窗 |
| `IdentitySelectDialog.tsx` | 身份选择弹窗 |

图表组件（`components/Charts/`）负责统计页的各类可视化：

| 组件 | 用途 |
|------|------|
| `ExpenseTrendChart.tsx` | 消费趋势折线图 |
| `CategoryPieChart.tsx` | 分类占比饼图 |
| `CategoryBarChart.tsx` | 分类柱状图 |
| `MerchantRankingChart.tsx` | 商户消费排行 |
| `MealDistChart.tsx` | 用餐时段分布 |
| `PositionPieChart.tsx` | 消费位置分布 |
| `ConsumptionDistributionChart.tsx` | 消费金额分布 |
| `MonthComparisonCard.tsx` | 月度对比卡片 |

## 2. Tauri 命令层

目录在 `src-tauri/src/commands/`，作为前端和 Rust 服务之间的桥。

| 命令模块 | 注册的命令 | 职责 |
|---------|-----------|------|
| `identity.rs` | `list_identities`, `create_identity`, `update_identity`, `delete_identity`, `set_default_identity`, `get_default_identity`, `set_last_identity`, `get_last_identity` | 身份 CRUD 与默认身份管理 |
| `account.rs` | `list_accounts`, `create_account`, `update_account`, `delete_account` | 账号 CRUD |
| `bill.rs` | `query_bills`, `get_bill_detail`, `delete_merged_bill`, `update_bill_notes`, `dedupe_identity_bills`, `dedupe_account_bills`, `rebuild_merged_bills` | 账单查询、去重、重建 |
| `sync.rs` | `incremental_sync`, `full_sync`, `incremental_sync_account`, `full_sync_account`, `get_sync_progress`, `cas_login`, `check_login_status`, `sync_with_captcha`, `refresh_captcha` | 同步流程与手动验证码续传 |
| `captcha.rs` | `get_captcha_image`, `get_captcha_with_execution`, `get_local_ocr_model_status`, `ensure_local_ocr_models`, `cancel_local_ocr_model_download`, `delete_local_ocr_models`, `test_captcha`, `batch_test_captcha`, `init_local_ocr`, `unload_local_ocr` | 验证码获取、OCR 模型管理、测试 |
| `data.rs` | `export_data`, `import_data`, `list_snapshots`, `create_snapshot`, `restore_snapshot` | 数据导入导出与快照 |
| `config.rs` | `load_config`, `save_config`, `verify_startup_password`, `set_startup_password`, `get_app_version`, `check_for_updates`, `get_auto_sync_status`, `get_session_expiration_status`, `check_session_expiration`, `restart_session_expiration_service` | 配置管理与更新检查 |
| `statistics.rs` | `get_statistics_summary`, `get_daily_trend`, `get_category_distribution`, `get_meal_distribution`, `get_consumption_distribution`, `get_merchant_ranking`, `get_category_summary`, `get_forgot_card_stats`, `get_category_bills` | 各类统计查询 |
| `classify.rs` | `translate_target`, `classify_bill`, `get_bill_statistics`, `get_classification_rules` | 分类与翻译 |
| `error.rs` | `log_error` | 错误日志记录 |

命令层的设计原则是尽量薄：接收前端参数，调用 `AppState` 中对应服务，返回前端需要的结构。命令层不包含业务逻辑。

## 3. Rust 服务与数据层

目录结构：

| 目录/文件 | 职责 |
|----------|------|
| `state.rs` | `AppState`：全局状态容器，持有所有服务实例 |
| `sync/mod.rs` | `BillSyncService`：账单同步核心服务 |
| `auto_sync.rs` | `AutoSyncService`：自动同步定时任务 |
| `session_refresh.rs` | `SessionExpirationService`：会话过期检查与续期 |
| `db/init.rs` | 数据库初始化与连接管理 |
| `db/store.rs` | `BillStoreImpl`：账单落库、重建、去重 |
| `database/mod.rs` | `DatabaseFileManager`：规则与映射文件管理 |
| `entity/` | 数据实体定义（identities, accounts, bill_original, bill_merged, session_info, operation_log） |
| `config/mod.rs` | `AppConfig` + `TomlConfig`：配置结构与 TOML 管理 |
| `crypto/mod.rs` | `CryptoService`：加密解密与密码哈希 |
| `export/mod.rs` | `ExportService`：快照、导入导出 |
| `classification/mod.rs` | `BillClassifier`：账单分类引擎 |
| `models.rs` | 共享数据模型（Account 等） |
| `error.rs` | 统一错误类型 `AppError` |

## 当前主入口

前端入口：

- `src/main.tsx` — React 挂载点
- `src/App.tsx` — 根组件
- `src/components/Common/AppProvider.tsx` — 全局状态初始化

Rust 入口：

- `src-tauri/src/lib.rs` — `run()` 函数

`lib.rs` 启动时负责：

1. 初始化日志（`tracing_subscriber`，默认 `info` 级别）
2. 解析数据目录（Tauri `app_data_dir`）
3. 处理旧版数据迁移（`Data/` 目录 → Tauri 标准数据目录）
4. 创建 Tokio 运行时
5. 初始化 `AppState`（数据库、加密、配置、同步服务、分类器、会话服务等）
6. 注册全部 Tauri 命令
7. 启动后台服务（session 过期检查、自动同步）

## UI 入口组织

实际主界面主要分成三个页面：

- **首页**（`HomePage`）：消费概览、趋势、最近账单
- **账单页**（`BillPage`）：账单查询、筛选、同步操作
- **功能大全**（`FeaturesPage`）：次级功能入口网格

其他能力大多通过弹窗进入：

| 弹窗 | 入口 |
|------|------|
| 设置弹窗 | 功能大全 / 顶栏更多菜单 |
| 统计弹窗 | 功能大全 |
| 身份管理弹窗 | 功能大全 / 顶栏身份菜单 |
| 数据传输弹窗 | 功能大全 |
| 验证码测试弹窗 | 功能大全 |
| 手动验证码弹窗 | 同步过程中自动弹出 |
| OCR 模型下载弹窗 | 验证码测试页 |

# 配置与存储

## 配置文件

配置核心在：

- `src-tauri/src/config/mod.rs`

程序用 `AppConfig` 描述配置结构，用 `TomlConfig` 管理加载和保存。配置文件为 `app_config.toml`，位于应用数据目录下。

### AppConfig 结构

`AppConfig` 包含以下顶层配置段：

| 配置段 | 对应类型 | 主要字段 |
|--------|---------|---------|
| `security` | `SecurityConfig` | `enable_startup_protection`, `password_hash` |
| `identity` | `IdentityConfig` | `remember_default`, `default_identity_id`, `last_identity_id` |
| `captcha` | `CaptchaConfig` | `mode`, `remote_ocr_host`, `remote_ocr_port`, `remote_ocr_http_url`, `onnx_model_path`, `ocr_retry_count` |
| `sync` | `SyncConfig` | `max_pages`, `early_stop_threshold`, `skip_graduated_accounts`, `auto_merge_after_sync`, `auto_sync_enabled`, `auto_sync_interval_minutes`, `auto_sync_range` |
| `data` | `DataConfig` | `data_directory`, `snapshot_keep_count` |
| `classification` | `ClassificationConfig` | `rules_path`, `rules_update_url` |
| `update` | `UpdateConfig` | `auto_check`, `check_interval_hours`, `last_check_time` |
| `ui` | `UiConfig` | `theme`, `language`, `decimal_places`, `home_trend_range`, `home_category_range` |
| `session` | `SessionConfig` | `refresh_interval_minutes`, `auto_refresh` |

### CaptchaMode 枚举

验证码模式决定了同步时如何处理登录验证码：

| 枚举值 | 含义 |
|--------|------|
| `Manual` | 手动输入验证码（默认） |
| `RemoteOcr` | 远程 TCP OCR 服务识别 |
| `RemoteOcrHttp` | 远程 HTTP OCR 服务识别 |
| `LocalOnnx` | 本地 ONNX 模型识别 |

### SyncConfig 关键参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `max_pages` | 100 | 单次同步最大翻页深度（全量同步时自动设为 1000） |
| `early_stop_threshold` | 5 | 遇到连续多少页旧数据时提前停止（全量同步时设为 u32::MAX） |
| `skip_graduated_accounts` | true | 是否跳过已毕业账号（根据 `graduation_date` 判断） |
| `auto_merge_after_sync` | true | 同步后是否自动合并到身份账单 |
| `auto_sync_enabled` | false | 是否启用自动同步 |
| `auto_sync_interval_minutes` | 60 | 自动同步间隔（分钟） |
| `auto_sync_range` | Month | 自动同步默认时间范围 |

### UiConfig 关键参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `theme` | light | 界面主题 |
| `language` | zh-CN | 界面语言 |
| `decimal_places` | 2 | 统计数值保留小数位数 |
| `home_trend_range` | week | 首页趋势图时间范围 |
| `home_category_range` | month | 首页分类图时间范围 |

## 配置在程序中的位置

配置不是一次性读完就结束，而是：

1. **启动时加载**：`AppState::init()` 中调用 `TomlConfig::load(data_dir)`
2. **存入 `AppState`**：以 `Arc<RwLock<TomlConfig>>` 形式持有，支持异步读写
3. **命令层按需读取或修改**：通过 `State<AppState>` 获取配置
4. **修改后自动持久化**：`TomlConfig::update()` 和 `TomlConfig::save()` 会写回 TOML 文件

`TomlConfig` 还提供了一些便捷方法：

- `verify_startup_password()` — 验证启动密码
- `set_startup_password()` — 设置启动密码并启用保护
- `disable_startup_protection()` — 禁用启动保护
- `classification_rules_path()` — 获取分类规则文件路径（未配置则默认 `classification_rules.toml`）
- `data_directory()` — 获取数据目录路径（未配置则默认 `Data`）
- `onnx_model_path()` — 获取 ONNX 模型路径（未配置则默认 `Data/models`）
- `reset_to_default()` — 重置为默认配置

## 数据库存储

数据库相关能力主要在：

- `src-tauri/src/db/init.rs` — 数据库初始化与连接
- `src-tauri/src/db/store.rs` — `BillStoreImpl`：账单落库与去重
- `src-tauri/src/entity/` — 数据实体定义

数据库使用 SQLite，通过 SeaORM 管理。

### 数据实体

| 实体 | 文件 | 说明 |
|------|------|------|
| `identities` | `entity/identities.rs` | 身份信息 |
| `accounts` | `entity/accounts.rs` | 账号信息（含加密密码） |
| `bill_original` | `entity/bill_original.rs` | 原始账单（每个账号一份） |
| `bill_merged` | `entity/bill_merged.rs` | 合并账单（按身份聚合） |
| `session_info` | `entity/session_info.rs` | 登录会话（加密存储） |
| `operation_log` | `entity/operation_log.rs` | 操作日志 |

### DatabaseManager 与 BillStoreImpl

- `DatabaseManager` — 总入口，负责数据库连接、实体 CRUD、会话管理、账号密码加解密
- `BillStoreImpl` — 更偏账单落库，实现了 `shmtu_cas::sync::BillStore` trait，负责写入原始账单、合并到身份账单、去重、重建

## 规则与数据库文件

程序还会维护一套额外的本地文件，位于数据目录下的 `database/bill/` 目录：

| 文件 | 说明 |
|------|------|
| `rules.toml` | 分类规则定义 |
| `position.toml` | 消费位置翻译映射 |
| `type.toml` | 消费类型映射 |
| `schedule.toml` | 时间段规则定义 |

这些文件通过 `DatabaseFileManager` 管理，功能包括：

- 检测本地文件是否存在
- 缺失时从 GitHub 远端自动下载
- 提供 `create_position_translator()` 创建位置翻译器
- 提供 `rules_path()` 获取规则文件路径
- 提供 `download_all()` 从远端更新所有文件

## 数据目录结构

```
<app_data_dir>/
├── app_config.toml          # 应用配置
├── database/
│   └── bill/
│       ├── rules.toml       # 分类规则
│       ├── position.toml    # 位置翻译
│       ├── type.toml        # 类型映射
│       └── schedule.toml    # 时段规则
├── models/                  # ONNX 模型（本地 OCR）
└── shmtu_terminal.db        # SQLite 数据库
```

## 快照

快照相关能力主要在：

- `src-tauri/src/export/mod.rs` — `ExportService`
- `src-tauri/src/commands/data.rs` — 快照相关命令

快照不是只备份某一张表，而是把程序关键数据整体打包，包括原始账单、合并账单、身份账号信息、会话信息、操作日志等。

快照生命周期：

1. **创建**：`create_snapshot` 命令，将当前数据库状态保存为快照文件
2. **列表**：`list_snapshots` 命令，查看所有快照
3. **恢复**：`restore_snapshot` 命令，从快照文件恢复数据库
4. **清理**：根据 `snapshot_keep_count` 配置自动清理最旧的快照

## 加密

`CryptoService`（`src-tauri/src/crypto/mod.rs`）负责：

- 账号密码加密存储（`encrypt` / `decrypt`）
- 密码哈希验证（启动密码保护）
- 基于 `shmtu-terminal-device-key` 作为密钥种子

## 普通用户常误解的点

- **规则文件变化不等于账单丢失**：规则只影响分类展示，原始账单数据不受影响
- **配置变化不等于数据库重置**：修改配置不会删除已有数据
- **快照恢复可能连配置状态一起回退**：快照包含的是数据库状态，但恢复后的配置可能与快照创建时不同
- **数据目录可以迁移**：从旧版 `Data/` 目录到 Tauri 标准数据目录的迁移在 `lib.rs` 启动时自动处理

# 配置与存储

## 配置文件

配置核心在：

- `src-tauri/src/config/mod.rs`

程序用 `AppConfig` 描述配置结构，用 `TomlConfig` 管理加载和保存。

配置大类包括：

- 安全
- 身份
- 验证码
- 同步
- 数据
- 分类
- 更新
- UI
- session

## 配置在程序中的位置

配置不是一次性读完就结束，而是：

- 启动时加载
- 存入 `AppState`
- 命令层按需读取或修改

## 数据库存储

数据库相关能力主要在：

- `src-tauri/src/db/init.rs`
- `src-tauri/src/db/store.rs`

其中：

- `DatabaseManager` 更偏总入口和实体管理
- `BillStoreImpl` 更偏账单落库、重建、去重等操作

## 规则与数据库文件

程序还会维护一套额外的本地文件：

- `rules.toml`
- `position.toml`
- `type.toml`
- `schedule.toml`

这些文件通过 `DatabaseFileManager` 管理，并可在缺失时从远端下载。

## 快照

快照相关能力主要在：

- `src-tauri/src/export/mod.rs`
- `src-tauri/src/commands/data.rs`

快照不是只备份某一张表，而是把程序关键数据整体打包。

## 普通用户常误解的点

- 规则文件变化不等于账单丢失
- 配置变化不等于数据库重置
- 快照恢复可能连配置状态一起回退

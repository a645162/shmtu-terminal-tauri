# 数据流与状态

## 前端状态中心

前端主要使用：

- `Zustand`

核心状态集中在：

- `src/stores/appStore.ts`

它负责维护：

- 当前身份
- 账号列表
- 账单列表
- 同步进度
- 配置
- 统计数据
- 各类弹窗开关

## 前端调用链

一条典型调用链是：

1. 页面触发操作
2. `appStore` 调用 `src/services/tauri.ts`
3. `tauri.ts` 使用 `invoke(...)`
4. Rust 命令层接收请求
5. 命令层调用 `AppState` 中对应服务
6. 结果回到前端并写入 store

## Rust 全局状态

`src-tauri/src/state.rs` 中的 `AppState` 当前持有：

- `db_manager`
- `crypto`
- `config`
- `sync_service`
- `export_service`
- `classifier`
- `db_file_manager`
- `session_expiration_service`
- `auto_sync_service`
- 本地 OCR 相关状态

这意味着 Tauri 命令基本不自己 new 服务，而是从 `State<AppState>` 中取。

## 为什么这样组织

好处：

- 前端逻辑集中
- Rust 服务生命周期统一
- 命令层更薄
- 后续扩展功能时不容易散掉

代价：

- `AppState` 会逐渐变大
- 命令层和服务层的关系需要持续保持边界

## 数据分层

当前至少存在下面几层数据：

- 配置数据
- 身份与账号数据
- 原始账单数据
- 合并账单数据
- 会话数据
- 统计结果缓存式前端状态

对普通用户最容易混淆的是：

- 原始账单
- 合并账单

程序很多“去重”“重建”类动作，本质上是在这两层之间做整理。

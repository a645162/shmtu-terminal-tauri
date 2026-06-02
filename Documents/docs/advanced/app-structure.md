# 应用结构

海大终端当前是一个 `Vite + React + Tauri v2` 应用，分成三层：

## 1. 前端界面层

目录主要在：

- `src/pages`
- `src/components`
- `src/stores`
- `src/services/tauri.ts`

职责：

- 渲染界面
- 响应用户操作
- 调用 Tauri 命令
- 维护前端全局状态

## 2. Tauri 命令层

目录主要在：

- `src-tauri/src/commands`

职责：

- 作为前端和 Rust 服务之间的桥
- 接收前端参数
- 调用应用状态中的服务
- 返回前端需要的结构

## 3. Rust 服务与数据层

目录主要在：

- `src-tauri/src/state.rs`
- `src-tauri/src/sync/mod.rs`
- `src-tauri/src/db`
- `src-tauri/src/export`
- `src-tauri/src/config`
- `src-tauri/src/classification`

职责：

- 维护全局服务实例
- 管理数据库和配置
- 同步账单
- 导入导出与快照
- 分类与规则加载

## 当前主入口

前端入口：

- `src/App.tsx`
- `src/components/Common/AppProvider.tsx`

Rust 入口：

- `src-tauri/src/lib.rs`

`lib.rs` 负责：

- 初始化日志
- 准备数据目录
- 初始化 `AppState`
- 注册全部 Tauri 命令

## UI 入口组织

实际主界面主要分成：

- 首页
- 账单
- 功能大全

其他能力大多通过弹窗进入，比如：

- 设置
- 统计
- 身份管理
- 数据传输
- 验证码测试

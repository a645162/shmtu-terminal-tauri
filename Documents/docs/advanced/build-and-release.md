# 构建与发布

## 开发环境准备

### 前置依赖

- Node.js（建议 18+）
- Rust 工具链（`rustup` 安装 stable 版本）
- Tauri v2 CLI

### 安装前端依赖

```bash
npm install
```

## 前端开发与构建

### 开发模式

启动 Vite 开发服务器（仅前端热更新，不含 Tauri 后端）：

```bash
npm run dev
```

启动 Tauri 开发模式（前后端联动，Rust 修改会自动重编译）：

```bash
npm run tauri dev
```

### 前端构建

```bash
npm run build
```

这会执行 TypeScript 编译（`tsc`）和 Vite 打包。

### 前端类型检查

```bash
npx tsc --noEmit
```

### Rust 类型检查与 Lint

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml
```

## 文档构建与发布

### 文档本地预览

```bash
npm run docs:dev
```

### 文档构建

```bash
npm run docs:build
```

构建产物在 `Documents/docs/.vitepress/dist/` 目录下。

### 文档发布

当前文档已拆成：

- **主仓库文档**（普通用户版 + 高级版本）：在 `Documents/docs` 目录
- **lib API 文档**：在 `src-tauri/vendor/shmtu-cas-rs/Documents/docs` 目录

### GitHub Pages 部署

主仓库 Pages workflow：

- `.github/workflows/docs-pages.yml`

部署流程：

1. 推送到 main 分支时自动触发
2. 安装依赖并构建 VitePress 文档
3. 部署到 GitHub Pages

关键配置：

- VitePress `base` 已做了按仓库名自动推导（见 `config.ts` 中的 `resolveBase()`）
- GitHub Pages 需要先在仓库设置里启用 `GitHub Actions` 作为 source
- 图片资源现在都已经有占位文件，后续直接替换即可

## Tauri 应用构建

### 开发构建

```bash
npm run tauri dev
```

### 生产构建

```bash
npm run tauri build
```

这会同时构建前端和 Rust 后端，生成安装包。构建产物在 `src-tauri/target/release/bundle/` 目录下。

### 构建产物

根据平台不同，产物格式如下：

| 平台 | 产物格式 |
|------|---------|
| Windows | `.msi` 安装包、`.exe`（NSIS） |
| macOS | `.dmg`、`.app` |
| Linux | `.deb`、`.AppImage` |

## 项目脚本一览

| 命令 | 说明 |
|------|------|
| `npm run dev` | 启动 Vite 开发服务器 |
| `npm run build` | 前端生产构建 |
| `npm run tauri dev` | Tauri 开发模式（前后端联动） |
| `npm run tauri build` | Tauri 生产构建（含打包） |
| `npm run docs:dev` | 文档开发预览 |
| `npm run docs:build` | 文档生产构建 |

## 发布时要注意什么

- GitHub Pages 需要先在仓库设置里启用 `GitHub Actions` 作为 source
- VitePress `base` 已经做了按仓库名自动推导，本地预览时 base 为 `/`，GitHub Actions 构建时自动适配
- 图片资源现在都已经有占位文件，后续直接替换即可
- Tauri 应用版本号在 `src-tauri/tauri.conf.json` 中管理
- 发布新版本前建议完整跑一遍：`npm run build` + `cargo clippy` + `npm run docs:build`

## 数据目录

### 开发时

- Tauri 开发模式下，数据目录通常在 `~/.local/share/com.shmtu.terminal/`（Linux）或对应平台的 AppData 目录
- 旧版程序使用项目根目录下的 `Data/` 目录，`lib.rs` 中有自动迁移逻辑

### 生产时

- 使用 Tauri 标准的 `app_data_dir`
- 首次启动时如果检测到旧版 `Data/` 目录，会自动迁移到新路径

## 谁需要看这页

这页更适合：

- 维护文档的人
- 需要发布静态站点的人
- 需要理解构建链路的人
- 参与开发的新成员

# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
# shmtu-terminal-tauri

## Linux AppImage bundling

On Ubuntu 24.04, Tauri's AppImage packaging path uses `linuxdeploy`, whose AppImage still looks for `libfuse.so.2`.
This repo routes `bun run tauri ...` through [`scripts/build/tauri-wrapper.mjs`](/home/konghaomin/Prj/SHMTU/shmtu-terminal/shmtu-terminal-tauri/scripts/build/tauri-wrapper.mjs), which sets `APPIMAGE_EXTRACT_AND_RUN=1` on Linux so bundling works without installing legacy FUSE 2 packages.

## Scripts Layout

- `scripts/build`: 桌面构建与打包脚本，包含 `build_linux.sh`、`build_macos.sh`、`build_windows.ps1`
- `scripts/dev`: Tauri 开发启动脚本
- `scripts/cas`: `shmtu-cas-rs` 相关工具入口
- `scripts/ocr`: OCR CLI / GUI / Server 入口
- `scripts/maintenance`: 清理数据、批量 `cargo update`、`chmod +x` 工具

# 构建与发布

## 前端与文档构建

主应用常用命令：

```bash
npm run build
```

用户文档构建：

```bash
npm run docs:build
```

## 文档发布

当前文档已拆成：

- 普通用户版与高级版本：都在主仓库 `Documents/docs`
- lib API 文档：在 `shmtu-cas-rs` 子仓库 `Documents/docs`

主仓库 Pages workflow：

- `.github/workflows/docs-pages.yml`

lib 仓库 Pages workflow：

- `src-tauri/vendor/shmtu-cas-rs/.github/workflows/docs-pages.yml`

## Tauri 运行入口

开发阶段常见入口：

```bash
npm run tauri dev
```

如果仓库里实际习惯是别的脚本，也可以继续沿用现有项目脚本。

## 发布时要注意什么

- GitHub Pages 需要先在仓库设置里启用 `GitHub Actions` 作为 source
- VitePress `base` 已经做了按仓库名自动推导
- 图片资源现在都已经有占位文件，后续直接替换即可

## 谁需要看这页

这页更适合：

- 维护文档的人
- 需要发布静态站点的人
- 需要理解构建链路的人

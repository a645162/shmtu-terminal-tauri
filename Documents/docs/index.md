---
layout: home

hero:
  name: 海大终端
  text: 两种版本的文档
  tagline: 给普通用户一条最短上手路径，也给高级用户一套更完整的技术与机制说明
  actions:
    - theme: brand
      text: 进入普通用户版
      link: /user/get-started
    - theme: alt
      text: 进入高级版本
      link: /advanced/overview
    - theme: alt
      text: 看详细说明
      link: /guide/quick-start

features:
  - title: 普通用户版更简单
    details: 每页只解决一个实际问题，比如第一次怎么用、怎么同步、怎么备份，不要求你理解参数和内部结构。
  - title: 高级版本更复杂
    details: 会讲应用结构、前后端分工、状态流、同步链路、配置与存储，适合维护者或重度用户。
  - title: 详细说明保留
    details: 你之前已经写好的较完整用户说明仍然保留，作为介于两者之间的详细版本。
  - title: 图片位已留好
    details: 现在所有引用都是真实图片路径，你后面直接覆盖同名截图即可。
---

## 先选你要看的版本

### 普通用户版

适合你只关心这些问题：

- 第一次怎么把软件用起来
- 账单怎么同步
- 统计怎么看
- 数据怎么备份

入口：

- [第一次使用](/user/get-started)
- [怎么同步账单](/user/sync-bills)
- [怎么备份和恢复](/user/backup-and-restore)

### 高级版本

适合你还想看这些内容：

- 这个桌面程序是怎么组织的
- 前端、Tauri 命令、Rust 服务层怎么分工
- 同步和验证码链路怎么运转
- 配置和数据落在哪里
- 怎么构建和发布

入口：

- [高级版本总览](/advanced/overview)
- [应用结构](/advanced/app-structure)
- [同步与验证码](/advanced/sync-and-captcha)

## 如果你想看更完整但不那么技术化的说明

可以继续看原来的详细说明：

- [快速开始](/guide/quick-start)
- [账单查询与同步](/guide/bills-and-sync)
- [设置与数据](/guide/settings-and-data)
- [统计分析](/guide/statistics-and-analysis)

## 截图占位目录

用户文档截图占位已放到：

- `Documents/docs/public/images/screenshots/`

当前已预留：

- `home/`
- `bill/`
- `identity/`
- `settings/`
- `statistics/`
- `sync/`
- `data/`

## 边界

- 这里仍然只讲 `tauri` 桌面程序本体
- `shmtu-cas-rs` 那套库级 API 文档仍在子仓库里单独维护

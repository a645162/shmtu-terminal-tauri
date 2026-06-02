---
layout: home

hero:
  name: 海大终端
  text: 用户文档
  tagline: 面向终端使用者的安装、配置、同步、统计与数据管理说明
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/quick-start
    - theme: alt
      text: 账单同步
      link: /guide/bills-and-sync
    - theme: alt
      text: 常见问题
      link: /guide/faq

features:
  - title: 第一次也能跑通
    details: 从身份创建、账号录入、验证码模式选择到首次同步，按用户流程组织文档，而不是按代码结构组织。
  - title: 同步策略讲清楚
    details: 解释增量同步、全量同步、单账号同步、人工验证码续传，以及不同时间范围的适用场景。
  - title: 数据操作更安全
    details: 导出、导入、快照、恢复、去重、重建分别会影响哪一层数据，都单独说明。
  - title: 截图位置预留好
    details: 已经为首页、账单页、统计页、设置页、同步与数据功能预留截图目录和命名建议。
---

## 文档范围

`海大终端` 是一个基于 `Tauri + React` 的桌面端工具，用来管理上海海事大学相关账号、同步校园消费账单、查看统计分析，并处理验证码识别、数据导入导出与快照备份。

这套文档面向普通使用者，重点解释：

- 软件能做什么
- 第一次应该怎么配置
- 同步账单时各模式如何选择
- 设置项分别会影响什么
- 数据导出、导入、快照恢复怎么安全操作

## 阅读建议

- 仅想先跑起来：看 [快速开始](/guide/quick-start)
- 主要关心同步：看 [账单查询与同步](/guide/bills-and-sync)
- 想理解设置项：看 [设置与数据](/guide/settings-and-data)
- 想核对统计含义：看 [统计分析](/guide/statistics-and-analysis)

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

## 使用边界

- 本文档只讲 `tauri` 桌面程序的用户操作。
- `lib` 与 OCR 服务的内部 API、模块设计、扩展方式，已拆分到 `shmtu-cas-rs` 子仓库的开发者文档中。

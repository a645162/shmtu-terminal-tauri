import { defineConfig } from 'vitepress'

function resolveBase() {
  const repo = process.env.GITHUB_REPOSITORY?.split('/')[1]
  if (!process.env.GITHUB_ACTIONS || !repo) {
    return '/'
  }
  return repo.endsWith('.github.io') ? '/' : `/${repo}/`
}

export default defineConfig({
  base: resolveBase(),
  lang: 'zh-CN',
  title: '海大终端文档',
  description: '海大终端 Tauri 版普通用户与高级使用文档',
  cleanUrls: true,
  lastUpdated: true,
  themeConfig: {
    nav: [
      { text: '普通用户版', link: '/user/get-started' },
      { text: '高级版本', link: '/advanced/overview' },
      { text: '详细说明', link: '/guide/quick-start' },
    ],
    sidebar: [
      {
        text: '普通用户版',
        items: [
          { text: '文档首页', link: '/' },
          { text: '第一次使用', link: '/user/get-started' },
          { text: '怎么同步账单', link: '/user/sync-bills' },
          { text: '怎么看统计', link: '/user/check-stats' },
          { text: '怎么备份和恢复', link: '/user/backup-and-restore' },
          { text: '常见问题', link: '/user/faq' },
        ],
      },
      {
        text: '高级版本',
        items: [
          { text: '总览', link: '/advanced/overview' },
          { text: '应用结构', link: '/advanced/app-structure' },
          { text: '数据流与状态', link: '/advanced/data-flow' },
          { text: '同步与验证码', link: '/advanced/sync-and-captcha' },
          { text: '配置与存储', link: '/advanced/config-and-storage' },
          { text: '构建与发布', link: '/advanced/build-and-release' },
        ],
      },
      {
        text: '详细说明',
        items: [
          { text: '快速开始', link: '/guide/quick-start' },
          { text: '界面总览', link: '/guide/interface-overview' },
          { text: '身份与账号管理', link: '/guide/identity-and-account' },
          { text: '账单查询与同步', link: '/guide/bills-and-sync' },
          { text: '统计分析', link: '/guide/statistics-and-analysis' },
          { text: '设置与数据', link: '/guide/settings-and-data' },
          { text: 'FAQ', link: '/guide/faq' },
        ],
      },
    ],
    outline: [2, 3],
    search: {
      provider: 'local',
    },
    footer: {
      message: 'SHMTU Terminal Tauri Docs',
      copyright: 'Copyright © SHMTU Terminal',
    },
  },
})

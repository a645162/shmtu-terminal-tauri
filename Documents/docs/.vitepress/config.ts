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
  title: '海大终端用户文档',
  description: '上海海事大学海大终端 Tauri 版使用说明',
  cleanUrls: true,
  lastUpdated: true,
  themeConfig: {
    nav: [
      { text: '快速开始', link: '/guide/quick-start' },
      { text: '账单同步', link: '/guide/bills-and-sync' },
      { text: '设置与数据', link: '/guide/settings-and-data' },
    ],
    sidebar: [
      {
        text: '开始使用',
        items: [
          { text: '文档首页', link: '/' },
          { text: '快速开始', link: '/guide/quick-start' },
          { text: '界面总览', link: '/guide/interface-overview' },
        ],
      },
      {
        text: '核心功能',
        items: [
          { text: '身份与账号管理', link: '/guide/identity-and-account' },
          { text: '账单查询与同步', link: '/guide/bills-and-sync' },
          { text: '统计分析', link: '/guide/statistics-and-analysis' },
          { text: '设置与数据', link: '/guide/settings-and-data' },
        ],
      },
      {
        text: '附录',
        items: [{ text: '常见问题', link: '/guide/faq' }],
      },
    ],
    outline: [2, 3],
    search: {
      provider: 'local',
    },
    footer: {
      message: 'SHMTU Terminal Tauri User Docs',
      copyright: 'Copyright © SHMTU Terminal',
    },
  },
})

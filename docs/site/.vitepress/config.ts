import { defineConfig } from 'vitepress'

export default defineConfig({
  lang: 'zh-CN',
  title: 'Fiex 项目文档',
  description: 'Fiex 桌面文件管理器的用户指南、架构说明与插件开发手册。',
  base: '/fiex/',
  cleanUrls: true,
  lastUpdated: true,
  themeConfig: {
    nav: [
      { text: '首页', link: '/' },
      { text: '用户指南', link: '/user-guide/start' },
      { text: '开发者文档', link: '/developer/overview' },
      { text: '参考', link: '/reference/security' },
      { text: 'GitHub', link: 'https://github.com/kevinhuang001/fiex' },
    ],
    sidebar: {
      '/user-guide/': [
        {
          text: '用户指南',
          items: [
            { text: '安装与首次启动', link: '/user-guide/start' },
            { text: '认识主窗口与浏览文件', link: '/user-guide/navigation' },
            { text: '选择、复制、移动与删除', link: '/user-guide/file-operations' },
            { text: '搜索与预览文件', link: '/user-guide/search-preview' },
            { text: '查看文件信息', link: '/user-guide/file-info' },
            { text: '常用位置、应用与废纸篓', link: '/user-guide/built-in-locations' },
            { text: '安装插件与连接服务', link: '/user-guide/plugins-and-connections' },
            { text: '设置与快捷键', link: '/user-guide/settings-shortcuts' },
            { text: '问题排查', link: '/user-guide/troubleshooting' },
          ],
        },
      ],
      '/developer/': [
        {
          text: '开发者文档',
          items: [
            { text: '项目与仓库', link: '/developer/overview' },
            { text: '本地开发环境', link: '/developer/environment' },
            { text: '系统架构', link: '/developer/architecture' },
            { text: 'IPC 协议参考', link: '/developer/ipc' },
            { text: '软件需求与测试追溯', link: '/developer/requirements' },
            { text: '格式、构建与 CI', link: '/developer/quality-ci' },
            { text: 'GitHub Pages 发布', link: '/developer/publishing' },
          ],
        },
        {
          text: '插件开发手册',
          collapsed: false,
          items: [
            { text: '开发路线与示例', link: '/developer/plugin-development' },
            { text: '新建并运行插件', link: '/developer/plugin-development/first-plugin' },
            { text: 'Manifest 与权限', link: '/developer/plugin-development/manifest' },
            { text: 'Provider 实现', link: '/developer/plugin-development/providers' },
            { text: '虚拟文件系统 VFS', link: '/developer/plugin-development/vfs' },
            { text: '插件界面与 UI SDK', link: '/developer/plugin-development/ui' },
            { text: '调试、验证与发布', link: '/developer/plugin-development/testing-publishing' },
          ],
        },
      ],
      '/reference/': [
        {
          text: '参考',
          items: [
            { text: '插件权限与安全边界', link: '/reference/security' },
            { text: '当前限制', link: '/reference/limitations' },
            { text: '文档代码基线', link: '/reference/source-baseline' },
          ],
        },
      ],
    },
    search: { provider: 'local' },
    socialLinks: [{ icon: 'github', link: 'https://github.com/kevinhuang001/fiex' }],
    footer: {
      message: 'Fiex 使用 GPL-3.0-or-later 许可证。',
      copyright: '文档随 Fiex 源代码共同维护。',
    },
  },
})

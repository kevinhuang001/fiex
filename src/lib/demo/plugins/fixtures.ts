import type { DemoPlugin } from './types'

const fixturePlugin: DemoPlugin = {
  id: 'org.test.fixture',
  name: '测试插件',
  version: '1.0.0',
  description: '用于验证右键子菜单、隔离 iframe 与上下文桥。',
  enabled: true,
  approved: true,
  backendAvailable: true,

  declaredPermissions: [],
  grantedPermissions: [],
  requestedNetworkDomains: [],
  allowedNetworkDomains: [],
  author: { name: 'Fiex Fixtures' },
  extensions: {
    toolbar: [
      {
        id: 'fixture.tool',
        title: '测试工具',
        icon: 'puzzle-piece',
        panel: { entry: 'ui/index.html', title: '测试工具' },
      },
    ],
    search: [
      {
        id: 'fixture.search',
        title: 'Fixture Index',
        icon: 'magnifying-glass',
        scope: 'global',
        command: 'search.query',
      },
    ],
    contextMenus: [
      {
        id: 'fixture.root',
        title: '测试插件',
        when: 'context.selection.count > 0',
        children: [
          {
            id: 'fixture.panel',
            title: '信息面板',
            panel: {
              entry: 'ui/index.html',
              title: '信息面板',
              defaultSize: { width: 620, height: 560 },
            },
          },
          { id: 'fixture.action', title: '检查文件', command: 'fixture.inspect' },
        ],
      },
    ],
  },
}

export const fixturePlugins: DemoPlugin[] = [
  fixturePlugin,
  {
    id: 'org.example.git',
    name: 'Git',
    version: '0.1.0',
    description: '源代码管理、提交历史、远程仓库与只读历史浏览。',
    enabled: true,
    approved: true,
    backendAvailable: true,

    declaredPermissions: [
      'folders.read',
      'folders.create',
      'folders.delete',
      'folders.rename',
      'files.read',
      'files.write',
      'network',
      'commands',
    ],
    grantedPermissions: [
      'folders.read',
      'folders.create',
      'folders.delete',
      'folders.rename',
      'files.read',
      'files.write',
      'network',
      'commands',
    ],
    requestedNetworkDomains: ['github.com'],
    allowedNetworkDomains: ['github.com'],
    requestedCommands: ['git'],
    allowedCommands: ['git'],
    author: { name: 'Fiex Contributors' },
    extensions: {
      vfs: [{ id: 'history', title: 'Git 历史', readOnly: true }],
      toolbar: [
        {
          id: 'git.main',
          title: '源代码管理',
          icon: 'git-branch',
          command: 'git.status',
        },
      ],
      settings: [
        { id: 'historyLimit', label: '历史提交显示条数', type: 'number', defaultValue: '50' },
      ],
    },
  },
  {
    id: 'org.example.checksum',
    name: '校验和',
    version: '0.1.0',
    description: '计算 SHA-256、SHA-512、SHA-1、MD5 与 CRC32。',
    enabled: true,
    approved: true,
    backendAvailable: true,

    declaredPermissions: ['folders.read', 'files.read'],
    grantedPermissions: ['folders.read', 'files.read'],
    requestedNetworkDomains: [],
    allowedNetworkDomains: [],
    author: { name: 'Fiex Contributors' },
    extensions: {
      contextMenus: [
        {
          id: 'checksum.calculate',
          title: '查看校验和',
          when: 'context.selection.isFile',
          command: 'checksum.calculate',
          resultView: 'checksum',
        },
      ],
    },
  },
]

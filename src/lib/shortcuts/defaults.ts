export const SHORTCUT_ACTIONS = [
  'quickLook',
  'search',
  'goToFolder',
  'copy',
  'cut',
  'paste',
  'undo',
  'duplicate',
  'newTab',
  'closeTab',
  'newFolder',
  'newFile',
  'rename',
  'trash',
  'back',
  'forward',
  'up',
  'refresh',
  'selectAll',
  'open',
  'settings',
  'toggleSidebar',
  'info',
] as const

export type ShortcutAction = (typeof SHORTCUT_ACTIONS)[number]
export type ShortcutStyle = 'mac' | 'win' | 'linux'
export type ShortcutMap = Record<ShortcutAction, string[]>

export const SHORTCUT_STYLES: { id: ShortcutStyle; name: string }[] = [
  { id: 'mac', name: 'macOS' },
  { id: 'win', name: 'Windows' },
  { id: 'linux', name: 'Linux' },
]

export const SHORTCUT_LABELS: Record<ShortcutAction, string> = {
  quickLook: '快速查看',
  search: '搜索',
  goToFolder: '前往文件夹',
  copy: '复制',
  cut: '剪切',
  paste: '粘贴',
  undo: '撤销',
  duplicate: '创建副本',
  newTab: '新建标签页',
  closeTab: '关闭标签页',
  newFolder: '新建文件夹',
  newFile: '新建文件',
  rename: '重新命名',
  trash: '移到废纸篓',
  back: '后退',
  forward: '前进',
  up: '上一级',
  refresh: '刷新',
  selectAll: '全选',
  open: '打开',
  settings: '打开设置',
  toggleSidebar: '显示 / 隐藏侧栏',
  info: '显示简介',
}

export const SHORTCUT_GROUPS: { title: string; actions: ShortcutAction[] }[] = [
  {
    title: '浏览',
    actions: ['quickLook', 'search', 'goToFolder', 'back', 'forward', 'up', 'refresh', 'selectAll'],
  },
  {
    title: '文件',
    actions: [
      'open',
      'rename',
      'newFolder',
      'newFile',
      'copy',
      'cut',
      'paste',
      'duplicate',
      'trash',
      'undo',
      'info',
    ],
  },
  { title: '窗口与标签页', actions: ['newTab', 'closeTab', 'toggleSidebar', 'settings'] },
]

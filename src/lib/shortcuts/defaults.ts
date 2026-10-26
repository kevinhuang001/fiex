import { t } from '@/i18n'
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
  get quickLook() {
    return t('快速查看')
  },
  get search() {
    return t('搜索')
  },
  get goToFolder() {
    return t('前往文件夹')
  },
  get copy() {
    return t('复制')
  },
  get cut() {
    return t('剪切')
  },
  get paste() {
    return t('粘贴')
  },
  get undo() {
    return t('撤销')
  },
  get duplicate() {
    return t('创建副本')
  },
  get newTab() {
    return t('新建标签页')
  },
  get closeTab() {
    return t('关闭标签页')
  },
  get newFolder() {
    return t('新建文件夹')
  },
  get newFile() {
    return t('新建文件')
  },
  get rename() {
    return t('重新命名')
  },
  get trash() {
    return t('移到废纸篓')
  },
  get back() {
    return t('后退')
  },
  get forward() {
    return t('前进')
  },
  get up() {
    return t('上一级')
  },
  get refresh() {
    return t('刷新')
  },
  get selectAll() {
    return t('全选')
  },
  get open() {
    return t('打开')
  },
  get settings() {
    return t('打开设置')
  },
  get toggleSidebar() {
    return t('显示 / 隐藏侧栏')
  },
  get info() {
    return t('显示简介')
  },
}

export const SHORTCUT_GROUPS: { title: string; actions: ShortcutAction[] }[] = [
  {
    get title() {
      return t('浏览')
    },
    actions: ['quickLook', 'search', 'goToFolder', 'back', 'forward', 'up', 'refresh', 'selectAll'],
  },
  {
    get title() {
      return t('文件')
    },
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
  {
    get title() {
      return t('窗口与标签页')
    },
    actions: ['newTab', 'closeTab', 'toggleSidebar', 'settings'],
  },
]

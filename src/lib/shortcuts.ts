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

function preset(entries: [ShortcutAction, ...string[]][]): ShortcutMap {
  return Object.fromEntries(entries.map(([action, ...keys]) => [action, keys])) as ShortcutMap
}

const mac = preset([
  ['quickLook', 'Space'],
  ['search', 'Meta+F'],
  ['goToFolder', 'Meta+Shift+G', 'Meta+L'],
  ['copy', 'Meta+C'],
  ['cut', 'Meta+X'],
  ['paste', 'Meta+V'],
  ['undo', 'Meta+Z'],
  ['duplicate', 'Meta+D'],
  ['newTab', 'Meta+T'],
  ['closeTab', 'Meta+W'],
  ['newFolder', 'Meta+Shift+N'],
  ['newFile', 'Meta+Alt+N'],
  ['rename', 'Enter', 'F2'],
  ['trash', 'Meta+Backspace'],
  ['back', 'Meta+[', 'Alt+ArrowLeft'],
  ['forward', 'Meta+]', 'Alt+ArrowRight'],
  ['up', 'Meta+ArrowUp', 'Alt+ArrowUp'],
  ['refresh', 'Meta+R', 'F5'],
  ['selectAll', 'Meta+A'],
  ['open', 'Meta+ArrowDown'],
  ['settings', 'Meta+,'],
  ['toggleSidebar', 'Meta+Alt+S'],
  ['info', 'Meta+I'],
])

const win = preset([
  ['quickLook', 'Space'],
  ['search', 'Ctrl+F'],
  ['goToFolder', 'Ctrl+L'],
  ['copy', 'Ctrl+C'],
  ['cut', 'Ctrl+X'],
  ['paste', 'Ctrl+V'],
  ['undo', 'Ctrl+Z'],
  ['duplicate', 'Ctrl+D'],
  ['newTab', 'Ctrl+T'],
  ['closeTab', 'Ctrl+W'],
  ['newFolder', 'Ctrl+Shift+N'],
  ['newFile', 'Ctrl+Alt+N'],
  ['rename', 'F2'],
  ['trash', 'Delete'],
  ['back', 'Alt+ArrowLeft'],
  ['forward', 'Alt+ArrowRight'],
  ['up', 'Alt+ArrowUp'],
  ['refresh', 'F5', 'Ctrl+R'],
  ['selectAll', 'Ctrl+A'],
  ['open', 'Enter'],
  ['settings', 'Ctrl+,'],
  ['toggleSidebar', 'Ctrl+B'],
  ['info', 'Alt+Enter'],
])

const linux = preset([
  ['quickLook', 'Space'],
  ['search', 'Ctrl+F'],
  ['goToFolder', 'Ctrl+L'],
  ['copy', 'Ctrl+C'],
  ['cut', 'Ctrl+X'],
  ['paste', 'Ctrl+V'],
  ['undo', 'Ctrl+Z'],
  ['duplicate', 'Ctrl+D'],
  ['newTab', 'Ctrl+T'],
  ['closeTab', 'Ctrl+W'],
  ['newFolder', 'Ctrl+Shift+N'],
  ['newFile', 'Ctrl+Alt+N'],
  ['rename', 'F2'],
  ['trash', 'Delete'],
  ['back', 'Alt+ArrowLeft'],
  ['forward', 'Alt+ArrowRight'],
  ['up', 'Alt+ArrowUp'],
  ['refresh', 'F5', 'Ctrl+R'],
  ['selectAll', 'Ctrl+A'],
  ['open', 'Enter'],
  ['settings', 'Ctrl+,'],
  ['toggleSidebar', 'F9'],
  ['info', 'Alt+Enter'],
])

const presets: Record<ShortcutStyle, ShortcutMap> = { mac, win, linux }

export function defaultShortcuts(style: ShortcutStyle): ShortcutMap {
  return Object.fromEntries(
    SHORTCUT_ACTIONS.map((action) => [action, [...presets[style][action]]]),
  ) as ShortcutMap
}

export function inferShortcutStyle(): ShortcutStyle {
  const navigatorInfo =
    typeof navigator === 'undefined'
      ? undefined
      : (navigator as Navigator & { userAgentData?: { platform?: string } })
  const platform =
    navigatorInfo?.userAgentData?.platform ||
    navigatorInfo?.platform ||
    navigatorInfo?.userAgent ||
    ''
  if (/mac|iphone|ipad/i.test(platform)) return 'mac'
  if (/win/i.test(platform)) return 'win'
  return 'linux'
}

const KEY_ALIASES: Record<string, string> = {
  ' ': 'Space',
  Spacebar: 'Space',
  Esc: 'Escape',
  Return: 'Enter',
  Left: 'ArrowLeft',
  Right: 'ArrowRight',
  Up: 'ArrowUp',
  Down: 'ArrowDown',
  Del: 'Delete',
}

const MODIFIER_KEYS = new Set([
  'Shift',
  'Control',
  'Alt',
  'Meta',
  'CapsLock',
  'Fn',
  'FnLock',
  'Hyper',
  'Super',
  'OS',
])

function normalizedKey(event: KeyboardEvent): string | null {
  if (event.isComposing) return null
  let key = event.key
  // Option/Alt often turns a letter into a symbol or a dead key on macOS.
  // Record the physical key in that case so Option-based shortcuts work and
  // can be re-recorded (for example Cmd+Option+N).
  const code = event.code || ''
  if (event.altKey && !/^[a-zA-Z0-9]$/.test(key)) {
    if (/^Key[A-Z]$/.test(code)) key = code.slice(3)
    else if (/^Digit[0-9]$/.test(code)) key = code.slice(5)
  }
  if (!key || MODIFIER_KEYS.has(key) || key === 'Dead' || key === 'Unidentified') return null
  if (KEY_ALIASES[key]) key = KEY_ALIASES[key]
  if (key.length === 1) return /[a-z]/.test(key) ? key.toUpperCase() : key
  return key
}

export function comboFromEvent(event: KeyboardEvent): string | null {
  const key = normalizedKey(event)
  if (!key) return null
  const modifiers: string[] = []
  if (event.metaKey) modifiers.push('Meta')
  if (event.ctrlKey) modifiers.push('Ctrl')
  if (event.altKey) modifiers.push('Alt')
  if (event.shiftKey) modifiers.push('Shift')
  return [...modifiers, key].join('+')
}

function displayKey(key: string, style: ShortcutStyle): string {
  const mac: Record<string, string> = {
    ArrowLeft: '←',
    ArrowRight: '→',
    ArrowUp: '↑',
    ArrowDown: '↓',
    Enter: '↩',
    Backspace: '⌫',
    Delete: '⌦',
    Escape: 'Esc',
    Tab: '⇥',
    Space: 'Space',
  }
  const other: Record<string, string> = {
    ArrowLeft: '←',
    ArrowRight: '→',
    ArrowUp: '↑',
    ArrowDown: '↓',
  }
  if (style === 'mac' && mac[key]) return mac[key]
  return other[key] || key
}

export function formatShortcut(combo: string, style: ShortcutStyle): string {
  const parts = combo.split('+')
  const key = parts.pop()!
  const modifiers = new Set(parts)
  if (style === 'mac') {
    const symbols: [string, string][] = [
      ['Ctrl', '⌃'],
      ['Alt', '⌥'],
      ['Shift', '⇧'],
      ['Meta', '⌘'],
    ]
    return (
      symbols
        .filter(([modifier]) => modifiers.has(modifier))
        .map(([, symbol]) => symbol)
        .join('') + displayKey(key, style)
    )
  }
  const names: [string, string][] = [
    ['Meta', style === 'win' ? 'Win' : 'Super'],
    ['Ctrl', 'Ctrl'],
    ['Alt', 'Alt'],
    ['Shift', 'Shift'],
  ]
  return [
    ...names.filter(([modifier]) => modifiers.has(modifier)).map(([, name]) => name),
    displayKey(key, style),
  ].join('+')
}

export function matchShortcut(event: KeyboardEvent, shortcuts: ShortcutMap): ShortcutAction | null {
  const combo = comboFromEvent(event)
  if (!combo) return null
  for (const action of SHORTCUT_ACTIONS) {
    if (shortcuts[action]?.includes(combo)) return action
  }
  return null
}

export function shortcutLabel(
  shortcuts: ShortcutMap,
  action: ShortcutAction,
  style: ShortcutStyle,
): string {
  const combo = shortcuts[action]?.[0]
  return combo ? formatShortcut(combo, style) : ''
}

export function sameShortcuts(a: ShortcutMap, b: ShortcutMap): boolean {
  return SHORTCUT_ACTIONS.every(
    (action) => (a[action] || []).join('\u0000') === (b[action] || []).join('\u0000'),
  )
}

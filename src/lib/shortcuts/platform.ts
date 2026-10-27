import { SHORTCUT_ACTIONS } from './defaults'
import type { ShortcutAction, ShortcutMap, ShortcutStyle } from './defaults'

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

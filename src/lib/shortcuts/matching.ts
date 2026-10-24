import { SHORTCUT_ACTIONS } from './defaults'
import type { ShortcutAction, ShortcutMap, ShortcutStyle } from './defaults'

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

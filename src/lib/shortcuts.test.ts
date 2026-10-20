import { describe, expect, it } from 'vitest'
import {
  SHORTCUT_ACTIONS,
  comboFromEvent,
  defaultShortcuts,
  formatShortcut,
  matchShortcut,
  sameShortcuts,
} from './shortcuts'

function key(key: string, init: Partial<KeyboardEvent> = {}): KeyboardEvent {
  return {
    key,
    code: init.code || '',
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    isComposing: false,
    ...init,
  } as KeyboardEvent
}

describe('shortcuts', () => {
  it('has no conflicting default bindings', () => {
    for (const style of ['mac', 'win', 'linux'] as const) {
      const seen = new Map<string, string>()
      for (const action of SHORTCUT_ACTIONS) {
        for (const combo of defaultShortcuts(style)[action]) {
          expect(seen.get(combo), `${style}: ${combo}`).toBeUndefined()
          seen.set(combo, action)
        }
      }
    }
  })
  it('normalizes and matches keyboard events', () => {
    expect(comboFromEvent(key('n', { metaKey: true, shiftKey: true, code: 'KeyN' }))).toBe(
      'Meta+Shift+N',
    )
    expect(comboFromEvent(key(' '))).toBe('Space')
    expect(comboFromEvent(key('ArrowLeft', { altKey: true }))).toBe('Alt+ArrowLeft')
    expect(comboFromEvent(key('Meta', { metaKey: true }))).toBeNull()
    expect(comboFromEvent(key('Dead', { altKey: true, metaKey: true, code: 'KeyN' }))).toBe(
      'Meta+Alt+N',
    )
    const bindings = defaultShortcuts('mac')
    expect(matchShortcut(key('f', { metaKey: true }), bindings)).toBe('search')
    expect(matchShortcut(key('f', { ctrlKey: true }), bindings)).toBeNull()
  })
  it('formats platform styles and compares maps', () => {
    expect(formatShortcut('Meta+Shift+N', 'mac')).toBe('⇧⌘N')
    expect(formatShortcut('Meta+Shift+N', 'win')).toBe('Win+Shift+N')
    expect(formatShortcut('Ctrl+Alt+N', 'linux')).toBe('Ctrl+Alt+N')
    expect(formatShortcut('Alt+ArrowLeft', 'mac')).toBe('⌥←')
    expect(sameShortcuts(defaultShortcuts('win'), defaultShortcuts('win'))).toBe(true)
    expect(sameShortcuts(defaultShortcuts('win'), defaultShortcuts('linux'))).toBe(false)
  })
})

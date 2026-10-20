import { useEffect, useState } from 'react'
import {
  SHORTCUT_ACTIONS,
  defaultShortcuts,
  inferShortcutStyle,
  type ShortcutMap,
  type ShortcutStyle,
} from '@/lib/shortcuts'
import type { Preferences } from '@/lib/types'

function defaultsFor(style: ShortcutStyle): Preferences {
  return {
    view: 'list',
    group: 'none',
    sort: 'name',
    descending: false,
    hidden: false,
    respectIgnore: false,
    terminal: 'default',
    theme: 'system',
    iconSize: 88,
    favorites: [],
    shortcutStyle: style,
    shortcuts: defaultShortcuts(style),
  }
}

function isStyle(value: unknown): value is ShortcutStyle {
  return value === 'mac' || value === 'win' || value === 'linux'
}

function load(): Preferences {
  const systemStyle = inferShortcutStyle()
  const fallback = defaultsFor(systemStyle)
  try {
    const stored = JSON.parse(
      localStorage.getItem('fiex:preferences') || '{}',
    ) as Partial<Preferences>
    const style = isStyle(stored.shortcutStyle) ? stored.shortcutStyle : systemStyle
    const shortcuts: ShortcutMap = defaultShortcuts(style)
    if (stored.shortcuts && typeof stored.shortcuts === 'object') {
      for (const action of SHORTCUT_ACTIONS) {
        const value = stored.shortcuts[action]
        if (Array.isArray(value))
          shortcuts[action] = value.filter(
            (binding): binding is string => typeof binding === 'string' && binding.length > 0,
          )
      }
    }
    return { ...fallback, ...stored, shortcutStyle: style, shortcuts }
  } catch {
    return fallback
  }
}

export function usePreferences() {
  const [prefs, setPrefs] = useState<Preferences>(load)
  useEffect(() => {
    localStorage.setItem('fiex:preferences', JSON.stringify(prefs))
    const apply = () =>
      document.documentElement.classList.toggle(
        'dark',
        prefs.theme === 'dark' ||
          (prefs.theme === 'system' && matchMedia('(prefers-color-scheme: dark)').matches),
      )
    apply()
    const media = matchMedia('(prefers-color-scheme: dark)')
    media.addEventListener('change', apply)
    return () => media.removeEventListener('change', apply)
  }, [prefs])
  return [prefs, (p: Partial<Preferences>) => setPrefs((v) => ({ ...v, ...p }))] as const
}

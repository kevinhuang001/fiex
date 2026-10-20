import { defaultDisplayIgnore, defaultOperationIgnore } from '@/lib/ignores'
import {
  SHORTCUT_ACTIONS,
  defaultShortcuts,
  inferShortcutStyle,
  type ShortcutMap,
  type ShortcutStyle,
} from '@/lib/shortcuts'
import type { Preferences, SidebarSection } from '@/lib/types'
import { useEffect, useState } from 'react'

function defaultsFor(style: ShortcutStyle): Preferences {
  return {
    view: 'list',
    group: 'none',
    sort: 'name',
    descending: false,
    hidden: false,
    terminal: 'default',
    ide: 'vscode',
    theme: 'system',
    fontSize: 11,
    displayIgnore: defaultDisplayIgnore(),
    compressIgnore: defaultOperationIgnore(),
    copyIgnore: defaultOperationIgnore(),
    sidebarOrder: ['places', 'favorites', 'tags', 'devices'],
    placeOrder: [],
    hiddenPlaces: [],
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
    const fontSize =
      typeof stored.fontSize === 'number' ? Math.min(18, Math.max(8, stored.fontSize)) : 11
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
    return {
      ...fallback,
      ...stored,
      fontSize,
      displayIgnore: Array.isArray(stored.displayIgnore)
        ? stored.displayIgnore
        : fallback.displayIgnore,
      compressIgnore: Array.isArray(stored.compressIgnore)
        ? stored.compressIgnore
        : fallback.compressIgnore,
      copyIgnore: Array.isArray(stored.copyIgnore) ? stored.copyIgnore : fallback.copyIgnore,
      sidebarOrder: Array.isArray(stored.sidebarOrder)
        ? (stored.sidebarOrder.filter((section): section is SidebarSection =>
            ['places', 'favorites', 'tags', 'devices'].includes(section),
          ) as SidebarSection[])
        : fallback.sidebarOrder,
      hiddenPlaces: Array.isArray(stored.hiddenPlaces)
        ? stored.hiddenPlaces
        : fallback.hiddenPlaces,
      shortcutStyle: style,
      shortcuts,
    }
  } catch {
    return fallback
  }
}

export function usePreferences() {
  const [prefs, setPrefs] = useState<Preferences>(load)
  useEffect(() => {
    localStorage.setItem('fiex:preferences', JSON.stringify(prefs))
  }, [prefs])
  useEffect(() => {
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
  }, [prefs.theme])
  return [prefs, (p: Partial<Preferences>) => setPrefs((v) => ({ ...v, ...p }))] as const
}

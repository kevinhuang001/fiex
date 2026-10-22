import { demo } from '@/lib/api'
import { defaultDisplayIgnore, defaultOperationIgnore } from '@/lib/ignores'
import {
  SHORTCUT_ACTIONS,
  defaultShortcuts,
  inferShortcutStyle,
  type ShortcutMap,
  type ShortcutStyle,
} from '@/lib/shortcuts'
import { TAG_PRESETS, normalizeTags } from '@/lib/tags'
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
    startPage: 'recent',
    displayIgnore: defaultDisplayIgnore(),
    compressIgnore: defaultOperationIgnore(),
    sidebarOrder: ['places', 'favorites', 'tags', 'devices', 'connections'],
    placeOrder: [],
    hiddenPlaces: [],
    favorites: [],
    tags: TAG_PRESETS.map((tag) => ({ ...tag })),
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
    ) as Partial<Preferences> & { customTags?: unknown }
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
    // The isolated demo fixture is a folder tree under Documents; always open
    // it there instead of the empty Recent page a fresh profile would show.
    const startPage = demo
      ? 'documents'
      : stored.startPage === 'home' ||
          stored.startPage === 'documents' ||
          stored.startPage === 'last'
        ? stored.startPage
        : 'recent'
    // Profiles saved before the tag catalogue moved into Settings only have
    // `customTags`; fold them into the new single list, in front of the
    // built-in tags.
    const legacyTags = normalizeTags(stored.customTags) ?? []
    const tags = normalizeTags(stored.tags) ?? [
      ...TAG_PRESETS.map((tag) => ({ ...tag })),
      ...legacyTags,
    ]
    return {
      ...fallback,
      ...Object.fromEntries(
        Object.entries(stored).filter(
          ([key]) => key !== 'copyIgnore' && key !== 'customTags' && key !== 'tags',
        ),
      ),
      tags,
      startPage,
      fontSize,
      displayIgnore: Array.isArray(stored.displayIgnore)
        ? stored.displayIgnore
        : fallback.displayIgnore,
      compressIgnore: Array.isArray(stored.compressIgnore)
        ? stored.compressIgnore
        : fallback.compressIgnore,
      sidebarOrder: Array.isArray(stored.sidebarOrder)
        ? (() => {
            const known: SidebarSection[] = [
              'places',
              'favorites',
              'tags',
              'devices',
              'connections',
            ]
            const saved = stored.sidebarOrder.filter((section): section is SidebarSection =>
              known.includes(section),
            )
            // Older preferences predate the connections section; append it so
            // the new group appears without disturbing the saved order.
            return [...saved, ...known.filter((section) => !saved.includes(section))]
          })()
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

import { getLocale, normalizeLocale, setLocale } from '@/i18n'
import { call } from '@/lib/api'
import { applyTheme, installedThemes } from '@/lib/themes'
import { demo } from '@/lib/api'
import { defaultDisplayIgnore } from '@/lib/ignores'
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
    locale: getLocale(),
    view: 'list',
    group: 'none',
    sort: 'name',
    descending: false,
    hidden: false,
    terminal: 'default',
    ide: 'vscode',
    theme: 'system',
    themeId: '',
    backgroundImage: '',
    backgroundOpacity: 0.6,
    fontSize: 11,
    terminalFont: '',
    terminalFontSize: 13,
    terminalColor: '',
    startPage: 'recent',
    displayIgnore: defaultDisplayIgnore(),
    sidebarOrder: ['places', 'favorites', 'tags', 'devices', 'connections'],
    placeOrder: [],
    hiddenPlaces: [],
    hiddenSections: [],
    toolbarLeftOrder: ['group', 'sort', 'newFolder', 'ide', 'terminal'],
    toolbarRightOrder: ['refresh', 'split', 'activity'],
    restoreTabs: false,
    favorites: [],
    tags: TAG_PRESETS.map((tag) => ({ ...tag })),
    shortcutStyle: style,
    shortcuts: defaultShortcuts(style),
    pinnedPluginTools: [],
    syncInterval: 30,
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
    const backgroundOpacity =
      typeof stored.backgroundOpacity === 'number'
        ? Math.min(1, Math.max(0.05, stored.backgroundOpacity))
        : fallback.backgroundOpacity
    const terminalFont = typeof stored.terminalFont === 'string' ? stored.terminalFont : ''
    const terminalFontSize =
      typeof stored.terminalFontSize === 'number'
        ? Math.min(24, Math.max(9, stored.terminalFontSize))
        : fallback.terminalFontSize
    const terminalColor =
      typeof stored.terminalColor === 'string' && /^#[0-9a-fA-F]{6}$/.test(stored.terminalColor)
        ? stored.terminalColor
        : ''
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
    const tags = normalizeTags(stored.tags) ?? fallback.tags
    // In the plain Vite dev URL, show the built-in dog once as a demo. An
    // explicit backgroundImage (including "") in stored preferences wins.
    const demoDog =
      demo &&
      !new URLSearchParams(location.search).has('demo') &&
      !localStorage.getItem('fiex:background-removed')
        ? '/demo/dog.svg'
        : ''
    return {
      ...fallback,
      ...Object.fromEntries(Object.entries(stored).filter(([key]) => key in fallback)),
      locale: normalizeLocale(stored.locale),
      tags,
      startPage,
      fontSize,
      terminalFont,
      terminalFontSize,
      terminalColor,
      backgroundImage:
        typeof stored.backgroundImage === 'string' && stored.backgroundImage
          ? stored.backgroundImage
          : demoDog,
      backgroundOpacity,
      displayIgnore: Array.isArray(stored.displayIgnore)
        ? stored.displayIgnore
        : fallback.displayIgnore,
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
      hiddenSections: Array.isArray(stored.hiddenSections)
        ? stored.hiddenSections.filter((section): section is SidebarSection =>
            ['places', 'favorites', 'tags', 'devices', 'connections'].includes(section),
          )
        : fallback.hiddenSections,
      toolbarLeftOrder: Array.isArray(stored.toolbarLeftOrder)
        ? stored.toolbarLeftOrder.filter((item): item is string =>
            ['group', 'sort', 'newFolder', 'ide', 'terminal'].includes(item),
          )
        : fallback.toolbarLeftOrder,
      toolbarRightOrder: Array.isArray(stored.toolbarRightOrder)
        ? stored.toolbarRightOrder.filter((item): item is string =>
            ['refresh', 'split', 'activity'].includes(item),
          )
        : fallback.toolbarRightOrder,
      restoreTabs:
        typeof stored.restoreTabs === 'boolean' ? stored.restoreTabs : fallback.restoreTabs,
      shortcutStyle: style,
      shortcuts,
      syncInterval:
        typeof stored.syncInterval === 'number' && [30, 60, 300, 900].includes(stored.syncInterval)
          ? stored.syncInterval
          : fallback.syncInterval,
    }
  } catch {
    return fallback
  }
}

export function usePreferences() {
  const [prefs, setPrefs] = useState<Preferences>(load)
  useEffect(() => {
    setLocale(prefs.locale)
    void call('locale_set', { locale: prefs.locale })
      .catch(() => {})
      .then(() => window.dispatchEvent(new Event('fiex:plugins-changed')))
  }, [prefs.locale])
  useEffect(() => {
    localStorage.setItem('fiex:preferences', JSON.stringify(prefs))
  }, [prefs])
  useEffect(() => {
    // The desktop scheduler owns the timer; keep it in step with settings.
    void call('remote_set_sync_interval', { seconds: prefs.syncInterval }).catch(() => {})
  }, [prefs.syncInterval])
  useEffect(() => {
    let alive = true
    const media = matchMedia('(prefers-color-scheme: dark)')
    const apply = () => {
      const mode =
        prefs.theme === 'dark' || (prefs.theme === 'system' && media.matches) ? 'dark' : 'light'
      void installedThemes()
        .then((themes) => {
          if (alive)
            applyTheme(
              themes.find((theme) => theme.id === prefs.themeId),
              mode,
            )
        })
        .catch(() => {
          if (alive) applyTheme(undefined, mode)
        })
    }
    apply()
    media.addEventListener('change', apply)
    window.addEventListener('fiex:plugins-changed', apply)
    return () => {
      alive = false
      media.removeEventListener('change', apply)
      window.removeEventListener('fiex:plugins-changed', apply)
    }
  }, [prefs.theme, prefs.themeId])
  return [prefs, (p: Partial<Preferences>) => setPrefs((v) => ({ ...v, ...p }))] as const
}

import { useEffect, useMemo, useState } from 'react'
import { getLocale } from '@/i18n'
import type { ShortcutAction } from '@/lib/shortcuts'
import { call } from '@/lib/api'
import { validPluginMenuItems, type PluginMenu } from '@/components/plugins/PluginMenuNode'
import type { ContextMenuProps } from './types'

export interface CoreMenuItem {
  id: string
  kind: 'action'
  title: string
  icon: string
  shortcut?: ShortcutAction
  disabled?: boolean
  danger?: boolean
  placement?: 'tail'
  activity?: { label: string; kind: 'sync' | 'transfer'; quiet: boolean }
}
export interface CoreMenu {
  kind: 'builtin'
  providerId: string
  menuSessionId: string
  items: CoreMenuItem[]
}

interface CoreSnapshot {
  key: string
  menu: NonNullable<ContextMenuProps['menu']>
  menus: CoreMenu[]
}
interface PluginSnapshot {
  key: string
  menu: NonNullable<ContextMenuProps['menu']>
  menus: PluginMenu[]
}
interface MenuFailure {
  key: string
  menu: NonNullable<ContextMenuProps['menu']>
  error: string
}
const EMPTY_PLUGIN_MENUS: PluginMenu[] = []

/** Start Core and plugin resolution from the same right-click session. Core
 *  readiness controls the first-level menu; plugin results update only its
 *  fixed submenu entry and never change the first-level layout. */
export function useContextMenu({ menu, x }: Pick<ContextMenuProps, 'menu' | 'x'>) {
  const paths = menu?.entry
    ? [
        menu.entry.path,
        ...x.selected.filter(
          (path) => path !== menu.entry!.path && x.selected.includes(menu.entry!.path),
        ),
      ]
    : []
  const selectionKey = JSON.stringify(paths)
  const key = JSON.stringify([menu?.x, menu?.y, x.path, selectionKey, getLocale()])
  const sessionId = useMemo(() => crypto.randomUUID(), [key, menu])
  const [snapshot, setSnapshot] = useState<CoreSnapshot>()
  const [pluginSnapshot, setPluginSnapshot] = useState<PluginSnapshot>()
  const [failure, setFailure] = useState<MenuFailure>()
  const [pluginFailure, setPluginFailure] = useState<MenuFailure>()
  const [pluginRetry, setPluginRetry] = useState(0)

  useEffect(() => {
    if (!menu) return
    let active = true
    setFailure(undefined)
    void call<CoreMenu[]>('plugin_context_menu_resolve', {
      directory: x.path,
      paths: JSON.parse(selectionKey),
      source: 'core',
      sessionId,
    })
      .then((result) => {
        if (!Array.isArray(result)) throw new Error('Invalid Core context menu response')
        if (active) setSnapshot({ key, menu, menus: result })
      })
      .catch((error) => {
        if (active) setFailure({ key, menu, error: String(error) })
      })
    return () => {
      active = false
    }
  }, [key, menu, sessionId])

  useEffect(() => {
    if (!menu) return
    let active = true
    setPluginFailure(undefined)
    void call<PluginMenu[]>('plugin_context_menu_resolve', {
      directory: x.path,
      paths: JSON.parse(selectionKey),
      source: 'plugins',
      sessionId,
    })
      .then((result) => {
        if (!Array.isArray(result)) throw new Error('Invalid plugin menu response')
        const menus = result.filter(
          (candidate): candidate is PluginMenu =>
            !!candidate &&
            typeof candidate.pluginId === 'string' &&
            typeof candidate.menuSessionId === 'string' &&
            validPluginMenuItems(candidate.items),
        )
        if (active) setPluginSnapshot({ key, menu, menus })
      })
      .catch((error) => {
        if (active) setPluginFailure({ key, menu, error: String(error) })
      })
    return () => {
      active = false
    }
  }, [key, menu, pluginRetry, sessionId])

  const current = snapshot && snapshot.key === key && snapshot.menu === menu ? snapshot : undefined
  const currentFailure =
    failure && failure.key === key && failure.menu === menu ? failure.error : ''
  const currentPlugins =
    pluginSnapshot && pluginSnapshot.key === key && pluginSnapshot.menu === menu
      ? pluginSnapshot
      : undefined
  const currentPluginFailure =
    pluginFailure && pluginFailure.key === key && pluginFailure.menu === menu
      ? pluginFailure.error
      : ''

  return {
    paths,
    menus: current?.menus ?? [],
    pluginMenus: currentPlugins?.menus ?? EMPTY_PLUGIN_MENUS,
    pluginLoading: Boolean(menu) && !currentPlugins && !currentPluginFailure,
    pluginError: currentPluginFailure,
    retryPlugins: () => setPluginRetry((value) => value + 1),
    ready: Boolean(current || currentFailure),
    error: currentFailure,
  }
}

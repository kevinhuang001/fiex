import { useEffect, useMemo, useRef, useState } from 'react'
import { getLocale } from '@/i18n'
import type { ShortcutAction } from '@/lib/shortcuts'
import { SHORTCUT_ACTIONS } from '@/lib/shortcuts/defaults'
import type { CoreMenu, CoreMenuItem } from '@/lib/coreMenu'
import { call } from '@/lib/api'
import { validPluginMenuItems, type PluginMenu } from '@/components/plugins/PluginMenuNode'
import type { ContextMenuProps } from './types'

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

function parseCoreMenuItems(value: unknown): CoreMenuItem[] | null {
  if (!Array.isArray(value) || value.length > 100) return null
  const items: CoreMenuItem[] = []
  for (const valueItem of value) {
    if (!valueItem || typeof valueItem !== 'object' || Array.isArray(valueItem)) return null
    const item = valueItem as Record<string, unknown>
    if (
      typeof item.id !== 'string' ||
      typeof item.title !== 'string' ||
      !['action', 'submenu', 'separator'].includes(String(item.kind)) ||
      ('parentId' in item && typeof item.parentId !== 'string') ||
      ('icon' in item && typeof item.icon !== 'string') ||
      ('shortcut' in item &&
        (typeof item.shortcut !== 'string' ||
          !SHORTCUT_ACTIONS.includes(item.shortcut as ShortcutAction))) ||
      ('disabled' in item && typeof item.disabled !== 'boolean') ||
      ('danger' in item && typeof item.danger !== 'boolean') ||
      ('placement' in item && item.placement !== 'tail')
    )
      return null
    const activity = item.activity
    if ('activity' in item) {
      if (!activity || typeof activity !== 'object' || Array.isArray(activity)) return null
      const details = activity as Record<string, unknown>
      if (
        typeof details.label !== 'string' ||
        !['sync', 'transfer'].includes(String(details.kind)) ||
        typeof details.quiet !== 'boolean'
      )
        return null
    }
    const parsed: CoreMenuItem = {
      id: item.id,
      title: item.title,
      kind: item.kind as CoreMenuItem['kind'],
    }
    if (typeof item.parentId === 'string') parsed.parentId = item.parentId
    if (typeof item.icon === 'string') parsed.icon = item.icon
    if (typeof item.shortcut === 'string') parsed.shortcut = item.shortcut as ShortcutAction
    if (typeof item.disabled === 'boolean') parsed.disabled = item.disabled
    if (typeof item.danger === 'boolean') parsed.danger = item.danger
    if (item.placement === 'tail') parsed.placement = 'tail'
    if (activity && typeof activity === 'object' && !Array.isArray(activity)) {
      const details = activity as Record<string, unknown>
      parsed.activity = {
        label: details.label as string,
        kind: details.kind as 'sync' | 'transfer',
        quiet: details.quiet as boolean,
      }
    }
    items.push(parsed)
  }
  return items
}

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
  const requestedCoreChildren = useRef(new Set<string>())

  useEffect(() => {
    requestedCoreChildren.current.clear()
  }, [key])

  useEffect(() => {
    if (!menu) return
    let active = true
    setFailure(undefined)
    void call('plugin_context_menu_resolve', {
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
    void call('plugin_context_menu_resolve', {
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

  const resolveCoreChildren = async (group: CoreMenu, parentId: string) => {
    const requestKey = group.menuSessionId + ':' + parentId
    if (requestedCoreChildren.current.has(requestKey)) return
    requestedCoreChildren.current.add(requestKey)
    try {
      const response = await call('plugin_context_menu_children', {
        menuSessionId: group.menuSessionId,
        parentId,
      })
      const children = parseCoreMenuItems(response.items)
      if (!children) throw new Error('Invalid Core submenu response')
      setSnapshot((previous) => {
        if (!previous || previous.key !== key || previous.menu !== menu) return previous
        const updated = previous.menus.map((candidate) => {
          if (candidate.menuSessionId !== group.menuSessionId) return candidate
          const items = [...candidate.items, ...children]
          return validPluginMenuItems(items) ? { ...candidate, items } : candidate
        })
        return { ...previous, menus: updated }
      })
    } catch (error) {
      requestedCoreChildren.current.delete(requestKey)
      throw error
    }
  }

  return {
    paths,
    menus: current?.menus ?? [],
    pluginMenus: currentPlugins?.menus ?? EMPTY_PLUGIN_MENUS,
    pluginLoading: Boolean(menu) && !currentPlugins && !currentPluginFailure,
    pluginError: currentPluginFailure,
    retryPlugins: () => setPluginRetry((value) => value + 1),
    resolveCoreChildren,
    ready: Boolean(current || currentFailure),
    error: currentFailure,
  }
}

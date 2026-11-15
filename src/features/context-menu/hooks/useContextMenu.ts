import { useEffect, useMemo, useRef, useState } from 'react'
import { getLocale } from '@/i18n'
import type { CoreMenu } from '@/lib/coreMenu'
import { validPluginMenuItems, type PluginMenu } from '@/lib/pluginMenu'
import { contextMenuPaths, contextMenuRequestKey } from '../model/request'
import type { ContextMenuProps, ContextMenuSessionServices } from '../types'

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
export function useContextMenu(
  { menu, x }: Pick<ContextMenuProps, 'menu' | 'x'>,
  services: ContextMenuSessionServices,
) {
  const paths = useMemo(
    () => contextMenuPaths(menu?.entry?.path, x.selected),
    [menu?.entry?.path, x.selected],
  )
  const key = contextMenuRequestKey({
    x: menu?.x,
    y: menu?.y,
    directory: x.path,
    paths,
    locale: getLocale(),
  })
  const sessionRef = useRef<{
    key: string
    menu: ContextMenuProps['menu']
    id: string
  } | null>(null)
  if (!sessionRef.current || sessionRef.current.key !== key || sessionRef.current.menu !== menu) {
    sessionRef.current = { key, menu, id: services.newSessionId() }
  }
  const sessionId = sessionRef.current.id
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
    void services
      .resolveCoreMenus({
        directory: x.path,
        paths: [...paths],
        sessionId,
      })
      .then((result) => {
        if (active) setSnapshot({ key, menu, menus: result })
      })
      .catch((error) => {
        if (active) setFailure({ key, menu, error: String(error) })
      })
    return () => {
      active = false
    }
  }, [key, menu, paths, sessionId, services, x.path])

  useEffect(() => {
    if (!menu) return
    let active = true
    setPluginFailure(undefined)
    void services
      .resolvePluginMenus({
        directory: x.path,
        paths: [...paths],
        sessionId,
      })
      .then((result) => {
        if (active) setPluginSnapshot({ key, menu, menus: result })
      })
      .catch((error) => {
        if (active) setPluginFailure({ key, menu, error: String(error) })
      })
    return () => {
      active = false
    }
  }, [key, menu, paths, pluginRetry, sessionId, services, x.path])

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
      const children = await services.resolveCoreChildren(group.menuSessionId, parentId)
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

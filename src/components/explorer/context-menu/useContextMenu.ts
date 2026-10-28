import { useEffect, useState } from 'react'
import { getLocale } from '@/i18n'
import type { ShortcutAction } from '@/lib/shortcuts'
import { call } from '@/lib/api'
import type { PluginMenu } from '@/components/plugins/PluginMenuNode'
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
export type ResolvedMenu = CoreMenu | (PluginMenu & { kind: 'plugin' })

/** One resolve for all providers. No frontend feature predicates or registry. */
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
  const [snapshot, setSnapshot] = useState<{
    key: string
    menu: ContextMenuProps['menu']
    menus: ResolvedMenu[]
  }>()
  const [error, setError] = useState('')
  useEffect(() => {
    if (!menu) return
    let active = true
    setError('')
    void call<ResolvedMenu[]>('plugin_context_menu_resolve', {
      directory: x.path,
      paths: JSON.parse(selectionKey),
    })
      .then((menus) => {
        if (active) setSnapshot({ key, menus, menu })
      })
      .catch((error) => {
        if (active) setError(String(error))
      })
    return () => {
      active = false
    }
  }, [key, menu])
  return {
    paths,
    menus: snapshot?.key === key && snapshot.menu === menu ? snapshot.menus : [],
    error,
  }
}

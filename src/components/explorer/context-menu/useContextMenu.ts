import { useEffect, useState } from 'react'
import { getLocale } from '@/i18n'
import type { ShortcutAction } from '@/lib/shortcuts'
import { call } from '@/lib/api'
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
interface CoreFailure {
  key: string
  menu: NonNullable<ContextMenuProps['menu']>
  error: string
}

/** Resolve Core-owned rows only. Plugin contributions live in the lazy "插件"
 *  submenu, so the first visible top-level menu is complete and never grows
 *  when a plugin answers later. */
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
  const [snapshot, setSnapshot] = useState<CoreSnapshot>()
  const [failure, setFailure] = useState<CoreFailure>()
  useEffect(() => {
    if (!menu) return
    let active = true
    setFailure(undefined)
    void call<CoreMenu[]>('plugin_context_menu_resolve', {
      directory: x.path,
      paths: JSON.parse(selectionKey),
      source: 'core',
    })
      .then((menus) => {
        if (active) setSnapshot({ key, menu, menus })
      })
      .catch((error) => {
        if (active) setFailure({ key, menu, error: String(error) })
      })
    return () => {
      active = false
    }
  }, [key, menu])
  const current = snapshot && snapshot.key === key && snapshot.menu === menu ? snapshot : undefined
  const currentFailure =
    failure && failure.key === key && failure.menu === menu ? failure.error : ''
  return {
    paths,
    menus: current?.menus ?? [],
    ready: Boolean(current || currentFailure),
    error: currentFailure,
  }
}

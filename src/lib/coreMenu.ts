import type { ShortcutAction } from './shortcuts'

export interface CoreMenuItem {
  id: string
  kind: 'action' | 'submenu' | 'separator'
  title: string
  parentId?: string
  icon?: string
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

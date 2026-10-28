import type { ReactNode } from 'react'
import type { AppController } from '@/hooks/useAppController'
import type { useMenuPosition } from '@/lib/menuPosition'

export interface ContextAction {
  id?: string
  disabled?: boolean
  danger?: boolean
  label: string
  icon: ReactNode
  hint: string
  action: () => void
}
export interface ContextSubmenu {
  id?: string
  disabled?: boolean
  label: string
  icon: ReactNode
  hint: string
  submenu: { label: string; action: () => void }[]
}
export type ContextItem = ContextAction | ContextSubmenu

export type MenuPosition = ReturnType<typeof useMenuPosition>
export type SetMenu = AppController['setMenu']

/** The controller slice that drives the file context menu. */
export type ContextMenuProps = Pick<
  AppController,
  | 'menu'
  | 'hint'
  | 'open'
  | 'previewEntry'
  | 'setInfo'
  | 'openBatchRename'
  | 'platform'
  | 'openTags'
  | 'x'
  | 'isRemotePath'
  | 'run'
  | 'copy'
  | 'duplicate'
  | 'terminal'
  | 'notify'
  | 'recycle'
  | 'create'
  | 'createFile'
  | 'deletePermanently'
  | 'restoreFromTrash'
  | 'inTrash'
  | 'setMenu'
>

/** Everything needed to build the item list; menu is known to be open. */
export type ContextItemsProps = Omit<ContextMenuProps, 'setMenu' | 'menu'> & {
  menu: NonNullable<AppController['menu']>
}

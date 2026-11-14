import type { Dispatch, ReactNode, SetStateAction } from 'react'
import type { useMenuPosition } from '@/lib/menuPosition'
import type { Entry } from '@/lib/types'
import type { Platform } from '@/lib/platform'
import type { ShortcutAction } from '@/lib/shortcuts'
import type { Explorer } from '@/features/explorer/hooks/useExplorer'
import type { OfficeFileTemplate } from '@/features/explorer/operations/create'
import type { Run } from '@/features/explorer/operations/types'

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
export type MenuRequest = { x: number; y: number; entry: Entry | null }
export type SetMenu = Dispatch<SetStateAction<MenuRequest | null>>

/** The controller slice that drives the file context menu. */
export interface ContextMenuProps {
  menu: MenuRequest | null
  hint: (action: ShortcutAction) => string
  open: (entry: Entry) => void
  previewEntry: (entry: Entry) => void
  setInfo: Dispatch<SetStateAction<Entry | null>>
  openBatchRename: (paths?: string[]) => void
  platform: Platform
  openTags: (paths: string[]) => void
  x: Explorer
  isRemotePath: (path: string) => boolean
  run: Run
  copy: (moving: boolean, paths?: string[]) => void
  duplicate: (paths?: string[]) => void
  addWorkspace: (paths: string[]) => Promise<void>
  workspace: {
    items: Entry[]
    remove: (path: string) => void
    setOpen: (open: boolean) => void
  }
  notify: (message: string) => void
  recycle: (paths?: string[]) => void
  create: () => Promise<void>
  createFile: (template?: OfficeFileTemplate) => Promise<void>
  deletePermanently: (paths: string[]) => void
  emptyTrash: () => void
  restoreFromTrash: (paths: string[]) => void
  inTrash: boolean
  setMenu: SetMenu
}

/** Everything needed to build the item list; menu is known to be open. */
export type ContextItemsProps = Omit<ContextMenuProps, 'setMenu' | 'menu'> & {
  menu: MenuRequest
}

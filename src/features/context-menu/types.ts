import type { Dispatch, SetStateAction } from 'react'
import type { Channel } from '@tauri-apps/api/core'
import type { Entry } from '@/lib/types'
import type { OperationProgress } from '@/lib/types'
import type { CoreMenu, CoreMenuItem } from '@/lib/coreMenu'
import type { PluginMenu } from '@/lib/pluginMenu'
import type { ShortcutAction } from '@/lib/shortcuts'
import type { OfficeFileTemplate } from '@/features/file-operations/create'
import type { Run } from '@/features/file-operations/types'

export type MenuRequest = { x: number; y: number; entry: Entry | null }
export type SetMenu = Dispatch<SetStateAction<MenuRequest | null>>

export interface ContextMenuExecutionServices {
  progressChannel: (id: string) => Channel<OperationProgress> | null
  quietProgressChannel: () => Channel<OperationProgress>
  executeMenu: (
    menuSessionId: string,
    itemId: string,
    onProgress: Channel<OperationProgress>,
  ) => Promise<unknown>
  recentNavigationTarget: (alias: string) => Promise<{ parent: string; target: string }>
  copyText: (value: string) => Promise<void>
  refreshDirectories: (paths: string[]) => void
  reportFilesystemPermissionError: (error: unknown, context: string) => boolean
}

export interface ContextMenuSessionServices {
  newSessionId: () => string
  resolveCoreMenus: (input: {
    directory: string
    paths: string[]
    sessionId: string
  }) => Promise<CoreMenu[]>
  resolvePluginMenus: (input: {
    directory: string
    paths: string[]
    sessionId: string
  }) => Promise<PluginMenu[]>
  resolveCoreChildren: (menuSessionId: string, parentId: string) => Promise<CoreMenuItem[]>
}

/** The controller slice that drives the file context menu. */
export interface ContextMenuProps {
  menu: MenuRequest | null
  hint: (action: ShortcutAction) => string
  open: (entry: Entry) => void
  previewEntry: (entry: Entry) => void
  setInfo: Dispatch<SetStateAction<Entry | null>>
  openBatchRename: (paths?: string[]) => void
  openTags: (paths: string[]) => void
  x: {
    entries: Entry[]
    path: string
    selected: string[]
    navigateTo: (directory: string, target: string) => void
    beginRename: (path: string) => void
    setError: (message: string) => void
  }
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
  setMenu: SetMenu
}

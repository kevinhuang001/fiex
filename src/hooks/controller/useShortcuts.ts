import type { Explorer } from '@/hooks/useExplorer'
import { useExplorerShortcuts } from '@/hooks/useExplorerShortcuts'
import type { DialogState } from '@/lib/dialogs'
import { shortcutLabel } from '@/lib/shortcuts'
import type { Entry, Preferences } from '@/lib/types'
import type { Dispatch, SetStateAction } from 'react'
import type { ConflictState, Transfer } from './types'

/** Keyboard wiring: every explorer shortcut, the escape reset and the shortcut
 *  hints the UI labels buttons with. */
export function useShortcuts({
  prefs,
  x,
  left,
  first,
  dialog,
  conflict,
  settings,
  preview,
  info,
  tagOpen,
  connectionOpen,
  trashIssue,
  focusSearch,
  startPathEdit,
  copy,
  clipboard,
  transfer,
  undo,
  create,
  newWorkspaceTab,
  closeWorkspaceTab,
  duplicate,
  previewEntry,
  open,
  recycle,
  refreshAll,
  setSidebar,
  setInfo,
  setSettings,
  setMenu,
}: {
  prefs: Preferences
  x: Explorer
  left: Explorer
  first: Entry | undefined
  dialog: DialogState | null
  conflict: ConflictState
  settings: boolean
  preview: Entry | null
  info: Entry | null
  tagOpen: boolean
  connectionOpen: boolean
  trashIssue: boolean
  focusSearch: () => void
  startPathEdit: () => void
  copy: (moving: boolean) => void
  clipboard: { paths: string[]; moving: boolean } | null
  transfer: Transfer
  undo: () => void
  create: (folder: boolean) => Promise<void>
  newWorkspaceTab: () => void
  closeWorkspaceTab: (index: number) => void
  duplicate: () => void
  previewEntry: (entry: Entry) => void
  open: (entry: Entry) => void
  recycle: () => void
  refreshAll: () => void
  setSidebar: Dispatch<SetStateAction<boolean>>
  setInfo: Dispatch<SetStateAction<Entry | null>>
  setSettings: Dispatch<SetStateAction<boolean>>
  setMenu: Dispatch<SetStateAction<{ x: number; y: number; entry: Entry } | null>>
}) {
  const hint = (action: Parameters<typeof shortcutLabel>[1]) =>
    shortcutLabel(prefs.shortcuts, action, prefs.shortcutStyle)
  useExplorerShortcuts({
    shortcuts: prefs.shortcuts,
    blocked: Boolean(
      dialog || conflict || settings || preview || info || tagOpen || connectionOpen || trashIssue,
    ),
    handlers: {
      settings: () => setSettings(true),
      search: focusSearch,
      goToFolder: startPathEdit,
      selectAll: () => x.setSelected(x.entries.map((entry) => entry.path)),
      copy: () => copy(false),
      cut: () => copy(true),
      paste: () => {
        if (clipboard) transfer(clipboard.paths, x.path, clipboard.moving)
      },
      undo,
      newFolder: () => {
        void create(true)
      },
      newFile: () => {
        void create(false)
      },
      newTab: newWorkspaceTab,
      closeTab: () => closeWorkspaceTab(left.tab),
      duplicate,
      back: x.back,
      forward: x.forward,
      up: x.up,
      quickLook: (event) => {
        if (first && !event.repeat) previewEntry(first)
      },
      rename: () => {
        if (first) x.beginRename(first.path)
      },
      open: () => {
        if (first) open(first)
      },
      trash: () => recycle(),
      refresh: refreshAll,
      toggleSidebar: () => setSidebar((value) => !value),
      info: () => {
        if (first) setInfo(first)
      },
    },
    onEscape: () => {
      setMenu(null)
      x.setSelected([])
      x.setQuery('')
    },
  })
  return { hint }
}

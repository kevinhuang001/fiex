import { useExplorer } from '@/hooks/useExplorer'
import { usePreferences } from '@/hooks/usePreferences'
import { call, native } from '@/lib/api'
import type { Entry } from '@/lib/types'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect } from 'react'
import { useConnectionActions } from './controller/useConnectionActions'
import { useDialogs } from './controller/useDialogs'
import { useDropTargets } from './controller/useDropTargets'
import { useOpenActions } from './controller/useOpenActions'
import { useShortcuts } from './controller/useShortcuts'
import { useWorkspaceActions } from './controller/useWorkspaceActions'
import { useDesktopEnvironment } from './useDesktopEnvironment'
import { useFileOperations } from './useFileOperations'
import { usePaneLayout } from './usePaneLayout'
import { usePathEditor } from './usePathEditor'
import { useWorkspace } from './useWorkspace'
export function useAppController() {
  const [prefs, update] = usePreferences()
  const left = useExplorer(prefs)
  const {
    x,
    right,
    split,
    setSplit,
    active,
    setActive,
    paneWidth,
    setPaneWidth,
    panePaths,
    switchWorkspaceTab,
    newWorkspaceTab,
    closeWorkspaceTab,
    moveWorkspaceTab,
    openPane,
    closePane,
  } = usePaneLayout(prefs, left)
  const { terminals, ides, platform, trashPath } = useDesktopEnvironment()
  const workspace = useWorkspace()
  const selectedAll = split ? [...new Set([...left.selected, ...right.selected])] : left.selected

  const {
    sidebar,
    setSidebar,
    preview,
    setPreview,
    settings,
    setSettings,
    info,
    setInfo,
    dialog,
    setDialog,
    dialogValue,
    setDialogValue,
    dialogError,
    setDialogError,
    dialogBusy,
    setDialogBusy,
    toast,
    setToast,
    trashIssue,
    setTrashIssue,
    tagOpen,
    setTagOpen,
    connectionOpen,
    setConnectionOpen,
    tagPaths,
    setTagPaths,
    tagInitial,
    setTagInitial,
    menu,
    setMenu,
    searchInput,
    chosen,
    first,
    notify,
    navigatePreview,
  } = useDialogs(x)
  const chosenAll = split
    ? [
        ...new Map(
          [...left.entries, ...right.entries]
            .filter((entry) => selectedAll.includes(entry.path))
            .map((entry) => [entry.path, entry]),
        ).values(),
      ]
    : chosen

  useEffect(() => {
    x.setReadOnly(x.directoryReadOnly)
  }, [x.directoryReadOnly, x.setReadOnly])
  const writable = () => {
    if (!x.directoryReadOnly) return true
    x.setError('此文件系统只读')
    return false
  }

  const {
    busy,
    conflict,
    setConflict,
    resolveConflict,
    clipboard,
    setClipboard,
    run,
    create,
    recycle,
    deletePermanently,
    restoreFromTrash,
    emptyTrash,
    inTrash,
    transfer,
    copy,
    duplicate,
    undo,
  } = useFileOperations({ x, writable, notify, setDialog, trashPath })

  const { ejectDevice, ejectUsb, mountDevice, removeConnection, isRemotePath, displayName } =
    useConnectionActions({ x, left, right, run, notify, setDialog, trashPath })

  const {
    open,
    previewEntry,
    openTrash,
    openApplications,
    terminal,
    openIde,
    openTags,
    applyTags,
  } = useOpenActions({
    x,
    prefs,
    run,
    notify,
    writable,
    first,
    ides,
    isRemotePath,
    tagPaths,
    setTagPaths,
    setTagInitial,
    setTagOpen,
    setPreview,
    setTrashIssue,
    trashPath,
  })

  const {
    editingPath,
    pathDraft,
    setPathDraft,
    pathInput,
    setEditingPath,
    startPathEdit,
    submitPath,
    crumbs,
  } = usePathEditor({ x: left, prefs, displayName })

  const { addWorkspace, refreshAll, bookmarkPaths, favorite } = useWorkspaceActions({
    x,
    prefs,
    update,
    workspace,
    notify,
    first,
    left,
    right,
    split,
  })

  const { hint } = useShortcuts({
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
    searchInput,
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
  })

  const { dragging } = useDropTargets({ x, conflict, transfer, bookmarkPaths, addWorkspace })

  useEffect(() => {
    if (!menu) return
    const close = () => setMenu(null)
    document.addEventListener('click', close)
    document.addEventListener('keydown', close)
    return () => {
      document.removeEventListener('click', close)
      document.removeEventListener('keydown', close)
    }
  }, [menu])
  const control = (action: 'close' | 'minimize' | 'toggleMaximize') => {
    if (native)
      void getCurrentWindow()
        [action]()
        .catch((e) => x.setError(String(e)))
  }
  return {
    sidebar,
    dragging,
    prefs,
    x,
    update,
    favorite,
    control,
    openTrash,
    openApplications,
    setConnectionOpen,
    removeConnection,
    refreshAll,
    ejectDevice,
    ejectUsb,
    mountDevice,
    trashPath,
    setSidebar,
    displayName,
    hint,
    create,
    ides,
    openIde,
    terminal,
    setSettings,
    newWindow: () =>
      void call('new_window', { path: x.path }).catch((error) => x.setError(String(error))),
    startPathEdit,
    editingPath,
    submitPath,
    pathInput,
    pathDraft,
    setPathDraft,
    setEditingPath,
    searchInput,
    crumbs,
    open,
    openLeft: (entry: Entry) => open(entry, left),
    openRight: (entry: Entry) => open(entry, right),
    previewEntry,
    setMenu,
    transfer,
    bookmarkPaths,
    busy,
    conflict,
    setConflict,
    resolveConflict,
    chosen,
    chosenAll,
    selectedAll,
    clipboard,
    setClipboard,
    menu,
    setInfo,
    platform,
    openTags,
    isRemotePath,
    run,
    copy,
    duplicate,
    notify,
    recycle,
    deletePermanently,
    restoreFromTrash,
    emptyTrash,
    inTrash,
    preview,
    setPreview,
    navigatePreview,
    dialog,
    dialogBusy,
    setDialog,
    setDialogBusy,
    setDialogError,
    dialogValue,
    setDialogValue,
    dialogError,
    trashIssue,
    setTrashIssue,
    info,
    tagOpen,
    tagInitial,
    setTagOpen,
    applyTags,
    connectionOpen,
    settings,
    terminals,
    toast,
    setToast,
    workspace,
    addWorkspace,
    left,
    right,
    split,
    paneWidth,
    panePaths,
    setPaneWidth,
    setSplit,
    active,
    setActive,
    closePane,
    openPane,
    switchWorkspaceTab,
    newWorkspaceTab,
    closeWorkspaceTab,
    moveWorkspaceTab,
  }
}
export type AppController = ReturnType<typeof useAppController>

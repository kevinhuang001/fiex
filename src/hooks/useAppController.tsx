import { t } from '@/i18n'
import { useExplorer } from '@/hooks/useExplorer'
import { usePreferences } from '@/hooks/usePreferences'
import { call, native } from '@/lib/api'
import { basename } from '@/lib/files'
import { loadSavedTabs, saveTabs } from '@/lib/tabs'
import type { Entry } from '@/lib/types'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect, useState } from 'react'
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
  const [batchRename, setBatchRename] = useState<Entry[] | null>(null)
  const left = useExplorer(
    prefs,
    undefined,
    true,
    'left',
    prefs.restoreTabs ? loadSavedTabs() : undefined,
  )
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
    paneActives,
    viewReferences,
    switchWorkspaceTab,
    newWorkspaceTab,
    closeWorkspaceTab,
    moveWorkspaceTab,
    openPane,
    closePane,
  } = usePaneLayout(prefs, left)
  useEffect(() => {
    if (left.tabs.length) saveTabs(left.tabs, left.tab)
  }, [left.tabs, left.tab])
  useEffect(() => {
    if (left.path && left.path !== 'recent://') localStorage.setItem('fiex:lastPath', left.path)
  }, [left.path])
  const { terminals, ides, platform, trashPath } = useDesktopEnvironment()
  const workspace = useWorkspace()
  const retainedViews = JSON.stringify({
    paths: viewReferences,
    durable: [
      ...(prefs.restoreTabs ? left.tabs : []),
      ...prefs.favorites.map((f) => f.path),
      ...workspace.items.map((e) => e.path),
      ...(prefs.startPage === 'last' ? [left.path] : []),
    ],
  })
  useEffect(() => {
    if (!native) return
    const timer = window.setTimeout(() => {
      void call('vfs_retain_views', JSON.parse(retainedViews)).catch((error) =>
        left.setError(String(error)),
      )
    }, 150)
    return () => window.clearTimeout(timer)
  }, [retainedViews])
  const selectedAll = split ? [...new Set([...left.selected, ...right.selected])] : left.selected
  const focusSearch = () => {
    const selector =
      split && active === 'right'
        ? '.secondary-pane .search-box input'
        : '.main > .path-toolbar .search-box input'
    document.querySelector<HTMLInputElement>(selector)?.focus()
  }

  const {
    sidebar,
    setSidebar,
    preview,
    setPreview,
    settings,
    setSettings,
    pluginManager,
    setPluginManager,
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
    trashIssue,
    setTrashIssue,
    tagOpen,
    setTagOpen,
    connectionOpen,
    setConnectionOpen,
    connectionToEdit,
    setConnectionToEdit,
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
  const openBatchRename = () => {
    if (chosen.length > 1) setBatchRename(chosen)
  }
  const applyBatchRename = async (renames: { path: string; name: string }[]) => {
    // Two-phase rename: move every source to a unique temporary name first,
    // then to its final name. This supports swaps (a↔b) and avoids a target
    // colliding with another source that has not moved yet.
    const staged: { original: string; temp: string; name: string }[] = []
    const final: { path: string; original: string; temp: string }[] = []
    try {
      for (const [index, item] of renames.entries()) {
        const tempName = `.fiex-batch-${crypto.randomUUID().slice(0, 8)}-${index}`
        const temp = await call<string>('rename_entry', { path: item.path, name: tempName })
        staged.push({ original: item.path, temp, name: item.name })
      }
      for (const item of staged) {
        const path = await call<string>('rename_entry', { path: item.temp, name: item.name })
        final.push({ path, original: item.original, temp: item.temp })
      }
      setBatchRename(null)
      x.setSelected([])
      x.reload()
      notify(t('已重命名 {0} 个项目', final.length))
    } catch (error) {
      // Park finalized targets back under temporary names first, so swaps
      // (a↔b) do not collide when restoring.
      for (const item of final.reverse()) {
        await call('rename_entry', { path: item.path, name: basename(item.temp) }).catch(() => {})
      }
      for (const item of staged.reverse()) {
        await call('rename_entry', { path: item.temp, name: basename(item.original) }).catch(
          () => {},
        )
      }
      throw error
    }
  }

  useEffect(() => {
    x.setReadOnly(x.directoryReadOnly)
  }, [x.directoryReadOnly, x.setReadOnly])
  const writable = () => {
    if (!x.directoryReadOnly) return true
    x.setError(t('此文件系统只读'))
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
    pluginManager,
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
    openConnectionDialog: (connection: import('@/lib/types').RemoteConnection | null = null) => {
      setConnectionToEdit(connection)
      setConnectionOpen(true)
    },
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
    batchRename,
    setBatchRename,
    openBatchRename,
    applyBatchRename,
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
    connectionToEdit,
    settings,
    pluginManager,
    setPluginManager,
    terminals,
    workspace,
    addWorkspace,
    left,
    right,
    split,
    paneWidth,
    panePaths,
    paneActives,
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

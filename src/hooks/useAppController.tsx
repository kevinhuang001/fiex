import { t } from '@/i18n'
import { useExplorer } from '@/features/explorer/hooks/useExplorer'
import { batchRenameEntries } from '@/features/explorer/operations/batchRename'
import { usePreferences } from '@/hooks/usePreferences'
import { call, native } from '@/lib/api'
import { loadSavedTabs, saveTabs } from '@/lib/tabs'
import type { Entry, Preferences } from '@/lib/types'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect, useState } from 'react'
import { useDialogs } from './controller/useDialogs'
import { useDesktopEnvironment } from './useDesktopEnvironment'
import { useAppControllerActions } from './controller/useAppControllerActions'
import { usePaneLayout } from './usePaneLayout'
import { useRemoteActivity } from './useRemoteActivity'
import { useWorkspace } from './useWorkspace'
export function useAppController() {
  useRemoteActivity()
  const [prefs, applyPreferences] = usePreferences()
  const [batchRename, setBatchRename] = useState<Entry[] | null>(null)
  const left = useExplorer(
    prefs,
    undefined,
    true,
    'left',
    prefs.restoreTabs ? loadSavedTabs() : undefined,
  )
  const update = (patch: Partial<Preferences>) => {
    applyPreferences(patch)
  }
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

  const dialogs = useDialogs(x)
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
    tagOpen,
    setTagOpen,
    connectionOpen,
    setConnectionOpen,
    connectionToEdit,
    setConnectionToEdit,
    tagInitial,
    menu,
    setMenu,
    searchInput,
    chosen,
    first,
    notify,
    navigatePreview,
  } = dialogs
  const chosenAll = split
    ? [
        ...new Map(
          [...left.entries, ...right.entries]
            .filter((entry) => selectedAll.includes(entry.path))
            .map((entry) => [entry.path, entry]),
        ).values(),
      ]
    : chosen
  const openBatchRename = (paths = x.selected) => {
    const targets = x.entries.filter((entry) => paths.includes(entry.path))
    if (targets.length > 1) setBatchRename(targets)
  }
  const applyBatchRename = async (renames: { path: string; name: string }[]) => {
    const renamed = await batchRenameEntries(renames, (path, name) =>
      call('rename_entry', { path, name }),
    )
    setBatchRename(null)
    x.setSelected([])
    x.reload()
    notify(t('已重命名 {0} 个项目', renamed.length))
  }

  const actions = useAppControllerActions({
    x,
    left,
    right,
    prefs,
    update,
    workspace,
    notify,
    first,
    ides,
    trashPath,
    dialogs,
    focusSearch,
    newWorkspaceTab,
    closeWorkspaceTab,
    split,
  })

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
    if (native) {
      const window = getCurrentWindow()
      void window[action]().catch((e) => x.setError(String(e)))
    }
  }
  return {
    sidebar,
    prefs,
    x,
    update,
    ...actions,
    control,
    setConnectionOpen,
    openConnectionDialog: (connection: import('@/lib/types').RemoteConnection | null = null) => {
      setConnectionToEdit(connection)
      setConnectionOpen(true)
    },
    trashPath,
    setSidebar,
    ides,
    setSettings,
    newWindow: () =>
      void call('new_window', { path: x.path }).catch((error) => x.setError(String(error))),
    searchInput,
    openLeft: (entry: Entry) => actions.open(entry, left),
    openRight: (entry: Entry) => actions.open(entry, right),
    setMenu,
    chosen,
    chosenAll,
    selectedAll,
    batchRename,
    setBatchRename,
    openBatchRename,
    applyBatchRename,
    menu,
    setInfo,
    platform,
    notify,
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
    info,
    tagOpen,
    tagInitial,
    setTagOpen,
    connectionOpen,
    connectionToEdit,
    settings,
    pluginManager,
    setPluginManager,
    terminals,
    workspace,
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

import { useExplorer } from '@/features/explorer/hooks/useExplorer'
import { usePreferences } from '@/hooks/usePreferences'
import { call, native } from '@/lib/api'
import { loadSavedTabs } from '@/lib/tabs'
import type { Entry, Preferences } from '@/lib/types'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect } from 'react'
import { useDialogs } from './controller/useDialogs'
import { useDesktopEnvironment } from './useDesktopEnvironment'
import { useAppControllerActions } from './controller/useAppControllerActions'
import { usePaneLayout } from './usePaneLayout'
import { useRemoteActivity } from './useRemoteActivity'
import { useWorkspace } from './useWorkspace'
import { explorerServices } from '@/features/explorer/services/listingAdapters'
import { useAppSessionPersistence } from './controller/useAppSessionPersistence'
import {
  mergePaneSelections,
  selectedEntriesAcrossPanes,
} from '@/features/explorer/model/paneSelection'
export function useAppController() {
  useRemoteActivity()
  const [prefs, applyPreferences] = usePreferences()
  const left = useExplorer(
    prefs,
    explorerServices,
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
  } = usePaneLayout(prefs, left, explorerServices)
  const { terminals, ides, platform, trashPath } = useDesktopEnvironment()
  const workspace = useWorkspace()
  useAppSessionPersistence({
    tabs: left.tabs,
    activeTab: left.tab,
    path: left.path,
    viewReferences,
    durablePaths: [
      ...(prefs.restoreTabs ? left.tabs : []),
      ...prefs.favorites.map((f) => f.path),
      ...workspace.items.map((e) => e.path),
      ...(prefs.startPage === 'last' ? [left.path] : []),
    ],
    setError: left.setError,
  })
  const selectedAll = mergePaneSelections(split, left.selected, right.selected)
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
  const chosenAll = selectedEntriesAcrossPanes(
    split,
    left.entries,
    right.entries,
    chosen,
    selectedAll,
  )
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
  }, [menu, setMenu])
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

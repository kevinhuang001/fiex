import type { AppController } from '@/hooks/useAppController'
import type { ExplorerTabsProps } from './ExplorerTabs'
import type { ExplorerPathBarProps } from './ExplorerPathBar'
import type { ExplorerStatusBarProps } from './ExplorerStatusBar'
import type { ExplorerToolbarProps } from './ExplorerToolbar'
import type { ExplorerDialogsProps } from './ExplorerDialogs'
import type { ContextMenuProps } from './context-menu/types'
import type { Preferences } from '@/lib/types'

export function explorerTabsProps(
  app: AppController,
  displayName: ExplorerTabsProps['displayName'],
): ExplorerTabsProps {
  return {
    left: app.left,
    panePaths: app.panePaths,
    paneActives: app.paneActives,
    displayName,
    switchWorkspaceTab: app.switchWorkspaceTab,
    closeWorkspaceTab: app.closeWorkspaceTab,
    newWorkspaceTab: app.newWorkspaceTab,
    moveWorkspaceTab: app.moveWorkspaceTab,
    control: app.control,
    platform: app.platform,
  }
}

export function explorerPathBarProps(app: AppController): ExplorerPathBarProps {
  return {
    x: app.left,
    startPathEdit: app.startPathEdit,
    editingPath: app.editingPath,
    submitPath: app.submitPath,
    pathInput: app.pathInput,
    pathDraft: app.pathDraft,
    setPathDraft: app.setPathDraft,
    setEditingPath: app.setEditingPath,
    searchInput: app.searchInput,
    crumbs: app.crumbs,
    searchHint: app.hint('search'),
    split: app.split,
    closePane: app.closePane,
  }
}

export function explorerStatusBarProps(app: AppController): ExplorerStatusBarProps {
  return {
    x: app.x,
    chosenAll: app.chosenAll,
    selectedAll: app.selectedAll,
    clipboard: app.clipboard,
    setClipboard: app.setClipboard,
    workspace: app.workspace,
  }
}

export function explorerToolbarProps(
  app: AppController,
  prefs: Preferences,
  update: (patch: Partial<Preferences>) => void,
  onToggleSplit: () => void,
): ExplorerToolbarProps {
  return {
    sidebar: app.sidebar,
    setSidebar: app.setSidebar,
    x: app.x,
    displayName: app.displayName,
    prefs,
    update,
    hint: app.hint,
    create: app.create,
    ides: app.ides,
    terminals: app.terminals,
    openIde: app.openIde,
    terminal: app.terminal,
    setSettings: app.setSettings,
    setPluginManager: app.setPluginManager,
    refreshAll: app.refreshAll,
    split: app.split,
    onToggleSplit,
  }
}

export function explorerDialogProps(app: AppController): ExplorerDialogsProps {
  return {
    left: app.left,
    x: app.x,
    preview: app.preview,
    setPreview: app.setPreview,
    navigatePreview: app.navigatePreview,
    open: app.open,
    dialog: app.dialog,
    dialogBusy: app.dialogBusy,
    conflict: app.conflict,
    cancelConflict: app.cancelConflict,
    resolveConflict: app.resolveConflict,
    busy: app.busy,
    setDialog: app.setDialog,
    setDialogBusy: app.setDialogBusy,
    setDialogError: app.setDialogError,
    dialogValue: app.dialogValue,
    setDialogValue: app.setDialogValue,
    info: app.info,
    setInfo: app.setInfo,
    tagOpen: app.tagOpen,
    tagInitial: app.tagInitial,
    setTagOpen: app.setTagOpen,
    applyTags: app.applyTags,
    connectionOpen: app.connectionOpen,
    connectionToEdit: app.connectionToEdit,
    setConnectionOpen: app.setConnectionOpen,
    notify: app.notify,
    settings: app.settings,
    setSettings: app.setSettings,
    pluginManager: app.pluginManager,
    setPluginManager: app.setPluginManager,
    prefs: app.prefs,
    update: app.update,
    terminals: app.terminals,
    ides: app.ides,
  }
}

export function explorerContextMenuProps(app: AppController): ContextMenuProps {
  return {
    menu: app.menu,
    hint: app.hint,
    open: app.open,
    previewEntry: app.previewEntry,
    setInfo: app.setInfo,
    openBatchRename: app.openBatchRename,
    platform: app.platform,
    openTags: app.openTags,
    x: app.x,
    isRemotePath: app.isRemotePath,
    run: app.run,
    copy: app.copy,
    duplicate: app.duplicate,
    addWorkspace: app.addWorkspace,
    workspace: app.workspace,
    notify: app.notify,
    recycle: app.recycle,
    create: app.create,
    createFile: app.createFile,
    deletePermanently: app.deletePermanently,
    emptyTrash: app.emptyTrash,
    restoreFromTrash: app.restoreFromTrash,
    inTrash: app.inTrash,
    setMenu: app.setMenu,
  }
}

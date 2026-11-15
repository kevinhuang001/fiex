import type { AppController } from '@/hooks/useAppController'
import type { ExplorerTabsProps } from './ExplorerTabs'
import type { ExplorerPathBarProps } from '@/features/explorer/components/ExplorerPathBar'
import type { ExplorerStatusBarProps } from '@/features/explorer/components/ExplorerStatusBar'
import type { ExplorerToolbarProps } from './ExplorerToolbar'
import type { ExplorerDialogsProps } from './ExplorerDialogs'
import type { ContextMenuProps } from '@/features/context-menu/types'
import type { Preferences } from '@/lib/types'

type Bindings<Fields extends keyof AppController> = Pick<AppController, Fields>

export function explorerTabsProps(
  app: Bindings<
    | 'left'
    | 'panePaths'
    | 'paneActives'
    | 'switchWorkspaceTab'
    | 'closeWorkspaceTab'
    | 'newWorkspaceTab'
    | 'moveWorkspaceTab'
    | 'control'
    | 'platform'
  >,
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

export function explorerPathBarProps(
  app: Bindings<
    | 'left'
    | 'startPathEdit'
    | 'editingPath'
    | 'submitPath'
    | 'pathInput'
    | 'pathDraft'
    | 'setPathDraft'
    | 'setEditingPath'
    | 'searchInput'
    | 'crumbs'
    | 'hint'
    | 'split'
    | 'closePane'
  >,
): ExplorerPathBarProps {
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

export function explorerStatusBarProps(
  app: Bindings<'x' | 'chosenAll' | 'selectedAll' | 'clipboard' | 'setClipboard' | 'workspace'>,
): ExplorerStatusBarProps {
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
  app: Bindings<
    | 'sidebar'
    | 'setSidebar'
    | 'x'
    | 'displayName'
    | 'hint'
    | 'create'
    | 'ides'
    | 'terminals'
    | 'openIde'
    | 'terminal'
    | 'setSettings'
    | 'setPluginManager'
    | 'refreshAll'
    | 'split'
  >,
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

export function explorerDialogProps(
  app: Bindings<
    | 'left'
    | 'x'
    | 'preview'
    | 'setPreview'
    | 'navigatePreview'
    | 'open'
    | 'dialog'
    | 'dialogBusy'
    | 'conflict'
    | 'cancelConflict'
    | 'resolveConflict'
    | 'busy'
    | 'setDialog'
    | 'setDialogBusy'
    | 'setDialogError'
    | 'dialogValue'
    | 'setDialogValue'
    | 'info'
    | 'setInfo'
    | 'tagOpen'
    | 'tagInitial'
    | 'setTagOpen'
    | 'applyTags'
    | 'connectionOpen'
    | 'connectionToEdit'
    | 'setConnectionOpen'
    | 'notify'
    | 'settings'
    | 'setSettings'
    | 'pluginManager'
    | 'setPluginManager'
    | 'prefs'
    | 'update'
    | 'terminals'
    | 'ides'
  >,
): ExplorerDialogsProps {
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

export function explorerContextMenuProps(
  app: Bindings<
    | 'menu'
    | 'hint'
    | 'open'
    | 'previewEntry'
    | 'setInfo'
    | 'openBatchRename'
    | 'openTags'
    | 'x'
    | 'run'
    | 'copy'
    | 'duplicate'
    | 'addWorkspace'
    | 'workspace'
    | 'notify'
    | 'recycle'
    | 'create'
    | 'createFile'
    | 'deletePermanently'
    | 'emptyTrash'
    | 'restoreFromTrash'
    | 'setMenu'
  >,
): ContextMenuProps {
  return {
    menu: app.menu,
    hint: app.hint,
    open: app.open,
    previewEntry: app.previewEntry,
    setInfo: app.setInfo,
    openBatchRename: app.openBatchRename,
    openTags: app.openTags,
    x: app.x,
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
    setMenu: app.setMenu,
  }
}

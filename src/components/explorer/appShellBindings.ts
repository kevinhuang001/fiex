import type { AppController } from '@/hooks/useAppController'
import type { ExplorerTabsProps } from './ExplorerTabs'
import type { ExplorerPathBarProps } from './ExplorerPathBar'
import type { ExplorerStatusBarProps } from './ExplorerStatusBar'
import type { ExplorerToolbarProps } from './ExplorerToolbar'
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

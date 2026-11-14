import type { AppController } from '@/hooks/useAppController'
import type { ExplorerTabsProps } from './ExplorerTabs'
import type { ExplorerPathBarProps } from './ExplorerPathBar'
import type { ExplorerStatusBarProps } from './ExplorerStatusBar'

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

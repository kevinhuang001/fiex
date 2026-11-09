import type { CoreMenu, CoreMenuItem } from './useContextMenu'

const MORE_ACTION_IDS = new Set(['duplicate', 'delete'])

export function isMoreAction(id: string): boolean {
  return MORE_ACTION_IDS.has(id)
}

export interface MoreCoreAction {
  group: CoreMenu
  item: CoreMenuItem
}

export interface WorkspaceMenuAction {
  kind: 'add' | 'remove'
  paths: string[]
}

export function buildMoreMenuPlan(
  menus: CoreMenu[],
  selectedPaths: string[],
  workspacePaths: string[],
): { coreActions: MoreCoreAction[]; workspaceAction: WorkspaceMenuAction | null } {
  const workspace = new Set(workspacePaths)
  const remove = selectedPaths.length > 0 && selectedPaths.every((path) => workspace.has(path))
  const workspaceAction = selectedPaths.length
    ? {
        kind: remove ? ('remove' as const) : ('add' as const),
        paths: remove ? selectedPaths : selectedPaths.filter((path) => !workspace.has(path)),
      }
    : null
  const coreActions = menus.flatMap((group) =>
    group.items.filter((item) => isMoreAction(item.id)).map((item) => ({ group, item })),
  )
  return { coreActions, workspaceAction }
}

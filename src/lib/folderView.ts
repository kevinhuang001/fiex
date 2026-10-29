import type { Group, Sort, View } from './types'
import type { ListColumns } from './listColumns'
import { normalizeListColumns } from './listColumns'
import { stateSnapshot, saveState } from './stateStore'

export interface FolderViewState {
  view: View
  sort: Sort
  descending: boolean
  group: Group
  listColumns?: ListColumns
}
const KEY = 'folder.views'
const LIMIT = 200
const views: View[] = ['list', 'icons', 'gallery', 'columns']
const sorts: Sort[] = ['name', 'modified', 'size', 'kind']
const groups: Group[] = ['none', 'kind', 'modified', 'size']

function read(): Record<string, FolderViewState> {
  const saved = stateSnapshot<Record<string, FolderViewState>>(KEY, 'fiex:folder-views')
  return saved && !Array.isArray(saved) && typeof saved === 'object' ? saved : {}
}
export function loadFolderView(path: string): FolderViewState | null {
  const value = read()[path]
  if (
    !value ||
    !views.includes(value.view) ||
    !sorts.includes(value.sort) ||
    !groups.includes(value.group)
  )
    return null
  return {
    view: value.view,
    sort: value.sort,
    descending: Boolean(value.descending),
    group: value.group,
    ...(value.listColumns ? { listColumns: normalizeListColumns(value.listColumns) } : {}),
  }
}
export function saveFolderView(path: string, state: FolderViewState) {
  if (!path) return
  const values = read()
  delete values[path]
  values[path] = state
  const keys = Object.keys(values)
  while (keys.length > LIMIT) {
    delete values[keys.shift()!]
  }
  saveState(KEY, values)
}

import type { Group, Sort, View } from './types'

/** Per-folder presentation remembered across sessions. */
export interface FolderViewState {
  view: View
  sort: Sort
  descending: boolean
  group: Group
}

const KEY = 'fiex:folder-views'
const LIMIT = 2000

const views: View[] = ['list', 'icons', 'gallery', 'columns']
const sorts: Sort[] = ['name', 'modified', 'size', 'kind']
const groups: Group[] = ['none', 'kind', 'modified', 'size']

function read(): Record<string, FolderViewState> {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) || '{}')
    return value && typeof value === 'object' ? value : {}
  } catch {
    return {}
  }
}

function write(values: Record<string, FolderViewState>) {
  try {
    localStorage.setItem(KEY, JSON.stringify(values))
  } catch {
    // Presentation state is best effort.
  }
}

export function loadFolderView(path: string): FolderViewState | null {
  const value = read()[path]
  if (!value || typeof value !== 'object') return null
  if (!views.includes(value.view) || !sorts.includes(value.sort) || !groups.includes(value.group))
    return null
  return {
    view: value.view,
    sort: value.sort,
    descending: Boolean(value.descending),
    group: value.group,
  }
}

export function saveFolderView(path: string, state: FolderViewState) {
  if (!path) return
  const values = read()
  values[path] = state
  const keys = Object.keys(values)
  if (keys.length > LIMIT) {
    for (const key of keys.slice(0, keys.length - LIMIT)) delete values[key]
  }
  write(values)
}

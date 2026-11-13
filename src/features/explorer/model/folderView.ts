import type { Group, Preferences, Sort, View } from '@/lib/types'

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
const groups: Group[] = ['none', 'kind', 'modified', 'extension', 'size']

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
  const previousDefaults = value.group === 'none' && value.sort === 'name' && !value.descending
  return {
    view: value.view,
    sort: previousDefaults ? 'modified' : value.sort,
    descending: previousDefaults ? true : Boolean(value.descending),
    group: previousDefaults ? 'modified' : value.group,
  }
}

/** Resolve a pane's current presentation from its folder record. Unvisited
 * folders inherit the application defaults and provider-specific view. */
export function folderViewPreferences(
  prefs: Preferences,
  path: string,
  defaultView?: View | null,
): Preferences {
  const saved = loadFolderView(path)
  return saved ? { ...prefs, ...saved } : { ...prefs, view: defaultView ?? prefs.view }
}

/** Apply only presentation fields to a folder record, retaining the rest. */
export function patchFolderView(
  current: Pick<Preferences, 'view' | 'sort' | 'descending' | 'group'>,
  patch: Partial<Pick<Preferences, 'view' | 'sort' | 'descending' | 'group'>>,
): FolderViewState {
  return { ...current, ...patch }
}

/** Persist a toolbar change for the focused folder and pin the other pane's
 *  current view before the shared preference defaults change. */
export function saveActiveFolderView(
  active: 'left' | 'right',
  panes: Record<
    'left' | 'right',
    {
      path: string
      preferences: Pick<Preferences, 'view' | 'sort' | 'descending' | 'group'>
    }
  >,
  patch: Partial<Pick<Preferences, 'view' | 'sort' | 'descending' | 'group'>>,
) {
  const pane = panes[active]
  const other = panes[active === 'left' ? 'right' : 'left']
  if (other.path && other.path !== pane.path)
    saveFolderView(other.path, patchFolderView(other.preferences, {}))
  if (pane.path) saveFolderView(pane.path, patchFolderView(pane.preferences, patch))
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

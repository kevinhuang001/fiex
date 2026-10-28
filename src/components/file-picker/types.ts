export type SortKey = 'name' | 'modified' | 'size' | 'kind'
export type GroupKey = 'none' | 'kind' | 'modified' | 'size'

const SORT_KEY = 'fiex.picker.sort'
const GROUP_KEY = 'fiex.picker.group'
const DIRECTORY_KEY = 'fiex.picker.directory'

export function storedSort(): { key: SortKey; direction: 'asc' | 'desc' } {
  try {
    const value = JSON.parse(localStorage.getItem(SORT_KEY) || 'null')
    if (value && ['name', 'modified', 'size', 'kind'].includes(value.key))
      return { key: value.key, direction: value.direction === 'desc' ? 'desc' : 'asc' }
  } catch {
    // fall through
  }
  return { key: 'name', direction: 'asc' }
}

export function storedGroup(): GroupKey {
  try {
    const value = localStorage.getItem(GROUP_KEY)
    if (value && ['none', 'kind', 'modified', 'size'].includes(value)) return value as GroupKey
  } catch {
    // fall through
  }
  return 'none'
}

export function persistPickerSort(
  sort: { key: SortKey; direction: 'asc' | 'desc' },
  group: GroupKey,
) {
  try {
    localStorage.setItem(SORT_KEY, JSON.stringify(sort))
    localStorage.setItem(GROUP_KEY, group)
  } catch {
    // Preferences are best effort.
  }
}

/** The last folder the chooser showed, so it reopens where the user left off. */
export function storedDirectory(): string {
  try {
    return localStorage.getItem(DIRECTORY_KEY) || ''
  } catch {
    return ''
  }
}

export function persistPickerDirectory(path: string) {
  if (!path) return
  try {
    localStorage.setItem(DIRECTORY_KEY, path)
  } catch {
    // Preferences are best effort.
  }
}

export const dirname = (path: string) => {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return index > 0 ? path.slice(0, index) : path
}

export const join = (directory: string, name: string) =>
  `${directory.replace(/[/\\]$/, '')}/${name}`.replace(/\/+/g, '/')

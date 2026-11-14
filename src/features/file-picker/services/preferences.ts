import type { FilePickerMode } from '@/lib/filePicker'
import type {
  GroupKey,
  PickerPreferenceServices,
  PickerSort,
  PickerStorage,
  SortKey,
} from '../types'

const SORT_KEY = 'fiex.picker.sort'
const GROUP_KEY = 'fiex.picker.group'
const DIRECTORY_KEY = 'fiex.picker.directory'

export const browserPickerStorage: PickerStorage = {
  getItem: (key) => localStorage.getItem(key),
  setItem: (key, value) => localStorage.setItem(key, value),
  removeItem: (key) => localStorage.removeItem(key),
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value))
}

function isSortKey(value: unknown): value is SortKey {
  return typeof value === 'string' && ['name', 'modified', 'size', 'kind'].includes(value)
}

function isGroupKey(value: unknown): value is GroupKey {
  return typeof value === 'string' && ['none', 'kind', 'modified', 'size'].includes(value)
}

export function storedSort(storage: PickerStorage = browserPickerStorage): PickerSort {
  try {
    const value: unknown = JSON.parse(storage.getItem(SORT_KEY) || 'null')
    if (isRecord(value) && isSortKey(value.key))
      return { key: value.key, direction: value.direction === 'desc' ? 'desc' : 'asc' }
  } catch {
    // Fall back to the default when local storage is unavailable or malformed.
  }
  return { key: 'name', direction: 'asc' }
}

export function storedGroup(storage: PickerStorage = browserPickerStorage): GroupKey {
  try {
    const value = storage.getItem(GROUP_KEY)
    if (isGroupKey(value)) return value
  } catch {
    // Fall back to the default when local storage is unavailable.
  }
  return 'none'
}

export async function persistPickerSort(
  sort: PickerSort,
  group: GroupKey,
  services: PickerPreferenceServices,
): Promise<void> {
  try {
    await Promise.all([
      services.writeState({ key: 'sort', value: sort }),
      services.writeState({ key: 'group', value: group }),
    ])
    services.storage.removeItem(SORT_KEY)
    services.storage.removeItem(GROUP_KEY)
  } catch {
    services.storage.setItem(SORT_KEY, JSON.stringify(sort))
    services.storage.setItem(GROUP_KEY, group)
  }
}

/** The last folder the chooser showed, so it reopens where the user left off. */
export function storedDirectory(storage: PickerStorage = browserPickerStorage): string {
  try {
    return storage.getItem(DIRECTORY_KEY) || ''
  } catch {
    return ''
  }
}

export async function persistPickerDirectory(
  path: string,
  mode: FilePickerMode,
  services: PickerPreferenceServices,
): Promise<void> {
  if (!path) return
  const key = mode === 'saveFile' ? 'lastSaveDirectory' : 'lastOpenDirectory'
  try {
    await services.writeState({ key, value: path })
    services.storage.removeItem(DIRECTORY_KEY)
  } catch {
    services.storage.setItem(DIRECTORY_KEY, path)
  }
}

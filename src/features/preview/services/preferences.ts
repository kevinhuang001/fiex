import type { PreviewPreferences } from '../types'

const ORDER_KEY = 'fiex.preview.order'
const DISABLED_KEY = 'fiex.preview.disabled'

export interface PreviewPreferenceStorage {
  getItem(key: string): string | null
}

const browserStorage: PreviewPreferenceStorage = {
  getItem: (key) => localStorage.getItem(key),
}

function readList(storage: PreviewPreferenceStorage, key: string): string[] {
  try {
    const value: unknown = JSON.parse(storage.getItem(key) || '[]')
    return Array.isArray(value)
      ? value.filter((item): item is string => typeof item === 'string')
      : []
  } catch {
    return []
  }
}

export function readPreviewPreferences(
  storage: PreviewPreferenceStorage = browserStorage,
): PreviewPreferences {
  return {
    order: readList(storage, ORDER_KEY),
    disabled: readList(storage, DISABLED_KEY),
  }
}

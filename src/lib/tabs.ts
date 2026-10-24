export interface SavedTabs {
  paths: string[]
  active: number
}

const KEY = 'fiex:tabs'

export function loadSavedTabs(): SavedTabs | undefined {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) || 'null') as Partial<SavedTabs> | null
    if (!value || !Array.isArray(value.paths)) return undefined
    const paths = value.paths.filter(
      (path): path is string => typeof path === 'string' && path.length > 0,
    )
    if (!paths.length) return undefined
    return {
      paths,
      active: Math.min(Math.max(0, Number(value.active) || 0), paths.length - 1),
    }
  } catch {
    return undefined
  }
}

export function saveTabs(paths: string[], active: number) {
  if (!paths.length) return
  localStorage.setItem(KEY, JSON.stringify({ paths, active }))
}

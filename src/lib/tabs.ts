import { stateSnapshot, saveState } from './stateStore'

export interface SavedTabs {
  paths: string[]
  active: number
}
export function loadSavedTabs(): SavedTabs | undefined {
  const value = stateSnapshot<Partial<SavedTabs>>('navigation.tabs', 'fiex:tabs')
  if (!value || !Array.isArray(value.paths)) return undefined
  const paths = value.paths.filter(
    (path): path is string => typeof path === 'string' && path.length > 0,
  )
  if (!paths.length) return undefined
  return {
    paths,
    active: Math.min(Math.max(0, Number(value.active) || 0), paths.length - 1),
  }
}
export function saveTabs(paths: string[], active: number) {
  if (paths.length) saveState('navigation.tabs', { paths, active })
}

import type { Location } from './types'

const KEY = 'fiex:locations:v1'

/** Last known places, used to paint the sidebar before the async volume scan. */
export function readCachedLocations(): Location[] {
  try {
    const cached = JSON.parse(localStorage.getItem(KEY) || '[]')
    return Array.isArray(cached) ? (cached as Location[]) : []
  } catch {
    return []
  }
}

export function cacheLocations(locations: Location[]) {
  if (!locations.length) return
  try {
    localStorage.setItem(KEY, JSON.stringify(locations))
  } catch {
    /* private windows may reject storage */
  }
}

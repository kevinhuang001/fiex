import { call } from './api'
import type { Location } from './types'
const KEY = 'fiex:locations:v1'
let snapshot: Location[] = []
export function readCachedLocations(): Location[] {
  return snapshot
}
export async function loadCachedLocations(): Promise<Location[]> {
  const stored = await call<Location[] | null>('state_get', {
    namespace: 'explorer',
    key: 'locations',
  })
  if (stored) {
    snapshot = stored
    localStorage.removeItem(KEY)
    return stored
  }
  try {
    const old = JSON.parse(localStorage.getItem(KEY) || '[]')
    if (Array.isArray(old) && old.length) {
      await call('state_put', { namespace: 'explorer', key: 'locations', value: old })
      snapshot = old
      localStorage.removeItem(KEY)
    }
  } catch {
    /* malformed migration data is not a live location source */
  }
  return snapshot
}
export function cacheLocations(locations: Location[]) {
  if (!locations.length) return
  snapshot = locations
  void call('state_put', { namespace: 'explorer', key: 'locations', value: locations }).catch(
    () => {},
  )
}

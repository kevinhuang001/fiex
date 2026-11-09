import { call } from './api'
import type { Location } from './types'
let snapshot: Location[] = []
export function readCachedLocations(): Location[] {
  return snapshot
}
export async function loadCachedLocations(): Promise<Location[]> {
  const stored = await call('state_get', {
    namespace: 'explorer',
    key: 'locations',
  })
  snapshot = stored ?? []
  return snapshot
}
export function cacheLocations(locations: Location[]) {
  if (!locations.length) return
  snapshot = locations
  void call('state_put', { namespace: 'explorer', key: 'locations', value: locations }).catch(
    () => {},
  )
}

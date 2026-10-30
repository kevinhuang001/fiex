import { loadCachedLocations } from './locationCache'
import { call } from '@/lib/api'
import type { Location, RemoteConnection } from '@/lib/types'

interface PickerSidebarData {
  locations: Location[]
  connections: RemoteConnection[]
  loadedAt: number
}

let snapshot: PickerSidebarData | null = null
let pending: Promise<PickerSidebarData> | null = null

/** Locations and connections are stable enough to share between picker
 *  instances; fetching them once avoids the blank sidebar on every open. */
export function loadPickerSidebarData(): Promise<PickerSidebarData> {
  if (snapshot && Date.now() - snapshot.loadedAt < 30_000) return Promise.resolve(snapshot)
  if (pending) return pending
  pending = Promise.all([
    call<Location[]>('locations').catch(() => loadCachedLocations().catch(() => [] as Location[])),
    call<RemoteConnection[]>('remote_connections').catch(() => [] as RemoteConnection[]),
  ])
    .then(([locations, connections]) => {
      snapshot = { locations, connections, loadedAt: Date.now() }
      return snapshot
    })
    .finally(() => {
      pending = null
    })
  return pending
}

export function pickerSidebarSnapshot(): PickerSidebarData | null {
  return snapshot
}

/** Warm the cache while the app starts, before the first picker is opened. */
export function warmFilePickerData() {
  void loadPickerSidebarData().catch(() => {})
}

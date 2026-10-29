import { call } from '@/lib/api'
import { MemoryCache } from '@/lib/cache'
import type { Location, RemoteConnection } from '@/lib/types'

interface PickerSidebarData {
  locations: Location[]
  connections: RemoteConnection[]
  loadedAt: number
}
// Reuse the shared bounded frontend cache primitive for picker sidebar data.
const sidebar = new MemoryCache<PickerSidebarData>(1, 30_000)
const key = 'picker.sidebar'
export function loadPickerSidebarData(): Promise<PickerSidebarData> {
  return sidebar.get(key, async () => {
    const [locations, connections] = await Promise.all([
      call<Location[]>('locations'),
      call<RemoteConnection[]>('remote_connections'),
    ])
    return { locations, connections, loadedAt: Date.now() }
  })
}
export function pickerSidebarSnapshot(): PickerSidebarData | null {
  return sidebar.peek(key) ?? null
}
export function warmFilePickerData() {
  void loadPickerSidebarData().catch(() => {})
}

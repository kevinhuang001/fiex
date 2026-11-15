import type { RemoteConnection } from '@/lib/types'

export async function refreshConnections(
  loadConnections: () => Promise<unknown>,
  setConnections: (connections: RemoteConnection[]) => void,
  setConnectionsLoaded: (loaded: boolean) => void,
) {
  try {
    const connections = await loadConnections()
    setConnections(Array.isArray(connections) ? [...connections] : [])
  } catch {
    setConnections([])
  } finally {
    setConnectionsLoaded(true)
  }
}

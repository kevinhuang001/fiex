import type { RemoteConnection } from '@/lib/types'

export interface ConnectionState {
  connections: RemoteConnection[]
  connectionsLoaded: boolean
  refreshConnections: () => Promise<void>
}

export interface ConnectionServices {
  remoteConnections: () => Promise<unknown>
}

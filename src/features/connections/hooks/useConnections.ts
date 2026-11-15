import { useCallback, useEffect, useState } from 'react'
import type { RemoteConnection } from '@/lib/types'
import { refreshConnections } from '../services/refreshConnections'
import type { ConnectionServices } from '../types'

/** Owns the app-wide connection list shared by both Explorer panes. */
export function useConnections(services: ConnectionServices) {
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [connectionsLoaded, setConnectionsLoaded] = useState(false)
  const refresh = useCallback(
    () => refreshConnections(services.remoteConnections, setConnections, setConnectionsLoaded),
    [services],
  )

  useEffect(() => {
    void refresh()
  }, [refresh])

  return { connections, connectionsLoaded, refreshConnections: refresh }
}

import { useEffect, useRef } from 'react'
import { call, native } from '@/lib/api'
import type { useAppController } from './useAppController'

/** Restore saved connections through their provider's navigation result. */
export function useConnectionAutostart(app: ReturnType<typeof useAppController>) {
  const autostarted = useRef(false)
  useEffect(() => {
    if (!native || autostarted.current || !app.left.hasListing) return
    autostarted.current = true
    const sessions = app.left.connections.filter(
      (connection) => connection.autostart && connection.saved !== false,
    )
    for (const session of sessions) {
      void call<{ kind: 'vfs' | 'pluginView'; path?: string }>('connection_open', {
        id: session.id,
      })
        .then((target) => {
          const path = target.kind === 'vfs' ? target.path : `fiex-plugin-view://${session.id}`
          if (path && !app.left.tabs.includes(path)) {
            app.newWorkspaceTab()
            app.left.navigate(path)
          }
        })
        .catch((error) => app.left.setError(String(error)))
    }
  }, [app.left.hasListing, app.left.connections, app.left.tabs])
}

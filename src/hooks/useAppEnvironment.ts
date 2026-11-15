import { useEffect, useRef } from 'react'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { call, native } from '@/lib/api'
import { shouldRestoreAutostartSessions } from './autostartSessions'
import type { Explorer } from '@/features/explorer/hooks/useExplorer'
import type { Preferences } from '@/lib/types'

type AppEnvironmentBindings = {
  left: Pick<
    Explorer,
    'hasListing' | 'connectionsLoaded' | 'connections' | 'tabs' | 'navigate' | 'setError'
  >
  newWorkspaceTab: () => void
  setPluginManager: (open: boolean) => void
  prefs: Pick<Preferences, 'fontSize'>
}

/** Native window integration, independent of the explorer render tree. */
export function useAppEnvironment(app: AppEnvironmentBindings) {
  const autostarted = useRef(false)
  const {
    left: { hasListing, connectionsLoaded, connections, tabs, navigate, setError },
    newWorkspaceTab,
    setPluginManager,
    prefs: { fontSize },
  } = app
  useEffect(() => {
    const handleNavigate = (event: Event) => {
      const detail = (event as CustomEvent<{ path?: string; pane?: string }>).detail
      if (!detail?.path) return
      navigate(detail.path)
    }
    window.addEventListener('fiex:navigate', handleNavigate)
    return () => window.removeEventListener('fiex:navigate', handleNavigate)
  }, [navigate])

  useEffect(() => {
    const openStore = () => setPluginManager(true)
    window.addEventListener('fiex:open-plugin-store', openStore)
    return () => window.removeEventListener('fiex:open-plugin-store', openStore)
  }, [setPluginManager])

  useEffect(() => {
    if (
      !shouldRestoreAutostartSessions({
        native,
        alreadyStarted: autostarted.current,
        hasListing,
        connectionsLoaded,
      })
    )
      return
    autostarted.current = true
    const sessions = connections.filter(
      (connection) => connection.autostart && connection.saved !== false,
    )
    for (const session of sessions) {
      // Each provider resolves its own navigation target, including autostart.
      const path = session.mountPath
      if (tabs.includes(path)) continue
      void call('connection_open', { id: session.id })
        .then((target) => {
          newWorkspaceTab()
          navigate(target.kind === 'vfs' ? target.path! : `fiex-plugin-view://${session.id}`)
        })
        .catch((error) => setError(String(error)))
    }
  }, [hasListing, connectionsLoaded, connections, tabs, newWorkspaceTab, navigate, setError])

  // The label size is the reference size: every window is zoomed by the same
  // ratio so the preference scales the sidebar, toolbar, dialogs and file list
  // together instead of only enlarging file names.
  useEffect(() => {
    if (!native) return
    void getCurrentWebviewWindow()
      .setZoom(fontSize / 11)
      .catch(() => {})
  }, [fontSize])
}

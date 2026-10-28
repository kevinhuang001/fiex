import { useEffect, useRef } from 'react'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { native } from '@/lib/api'
import type { useAppController } from './useAppController'

/** Browser/native window integration, independent of the explorer render tree. */
export function useAppEnvironment(app: ReturnType<typeof useAppController>) {
  const autostarted = useRef(false)
  // Browser demo has no native webview zoom; scale the app shell instead so
  // the preference still resizes text, icons, menus and dialogs together.
  const uiScale = native ? 1 : app.prefs.fontSize / 11
  useEffect(() => {
    if (native) return
    const root = document.documentElement
    root.style.setProperty('--ui-scale', String(uiScale))
    root.style.setProperty('--ui-vw', `calc(100vw / ${uiScale})`)
    root.style.setProperty('--ui-vh', `calc(100vh / ${uiScale})`)
    return () => {
      root.style.removeProperty('--ui-scale')
      root.style.removeProperty('--ui-vw')
      root.style.removeProperty('--ui-vh')
    }
  }, [uiScale])
  useEffect(() => {
    const navigate = (event: Event) => {
      const detail = (event as CustomEvent<{ path?: string; pane?: string }>).detail
      if (!detail?.path) return
      app.left.navigate(detail.path)
    }
    window.addEventListener('fiex:navigate', navigate)
    return () => window.removeEventListener('fiex:navigate', navigate)
  }, [app.left])

  useEffect(() => {
    const openStore = () => app.setPluginManager(true)
    window.addEventListener('fiex:open-plugin-store', openStore)
    return () => window.removeEventListener('fiex:open-plugin-store', openStore)
  }, [app.setPluginManager])

  useEffect(() => {
    if (!native || autostarted.current || !app.left.hasListing) return
    autostarted.current = true
    const sessions = app.left.connections.filter(
      (connection) =>
        (connection.kind === 'ssh' || connection.pluginId === 'org.fiex.ssh') &&
        connection.autostart &&
        connection.saved !== false &&
        !app.left.tabs.includes(`fiex-terminal://${connection.id}`),
    )
    for (const session of sessions) {
      app.newWorkspaceTab()
      app.left.navigate(`fiex-terminal://${session.id}`)
    }
  }, [app.left.hasListing, app.left.connections, app.left.tabs])

  // The label size is the reference size: every window is zoomed by the same
  // ratio so the preference scales the sidebar, toolbar, dialogs and file list
  // together instead of only enlarging file names.
  useEffect(() => {
    if (!native) return
    void getCurrentWebviewWindow()
      .setZoom(app.prefs.fontSize / 11)
      .catch(() => {})
  }, [app.prefs.fontSize])
  return uiScale
}

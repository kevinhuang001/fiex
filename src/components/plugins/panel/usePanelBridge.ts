import { t } from '@/i18n'
import { getLocale, subscribeLocale } from '@/i18n'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { attachFiexPanel } from '../../../../sdk/ui/src/host'
import type { JsonValue } from '../../../../sdk/ui/src/index'
import { themeSnapshot } from '@/lib/themes'
import { call, native } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { createPanelDispatcher, initialPanelSize } from './panelDispatch'
import { finishActivity } from '@/hooks/useActivity'
import type { PluginPanelRequest } from './usePanelSession'
import { PanelResources } from './resources'
import { MemoryCache } from '@/lib/cache'

export { MAX_PANEL_HEIGHT, MAX_PANEL_WIDTH } from './panelDispatch'

export interface PanelBridge {
  frameRef: RefObject<HTMLIFrameElement | null>
  html: string
  sourceUrl: string
  ready: boolean
  error: string
  size: { width: number; height: number }
  attach: () => void
}

export interface PanelBridgeOptions {
  request: PluginPanelRequest
  active: boolean
  onClose: () => void
  onReveal: () => void
  onSettleReveal: () => void
  onRevealFallback: () => void
}

function reportPluginError(pluginId: string, source: string, message: string) {
  if (!message) return
  void call('ui_error_report', { pluginId, source, message: message.slice(0, 500) }).catch(() => {
    // Desktop normally replays the persisted error through ErrorDialogHost.
    // Fall back locally if the diagnostic service is unavailable.
    reportError(message, `plugins:${source}`)
  })
}

/** Loads one plugin panel document and connects its iframe to the sandboxed
 * MessageChannel bridge. The iframe is sandboxed to an opaque origin; its only
 * way to reach Core is this bridge, never Tauri's global invoke. */
export function usePanelBridge({
  request,
  active,
  onClose,
  onReveal,
  onSettleReveal,
  onRevealFallback,
}: PanelBridgeOptions): PanelBridge {
  const activeRef = useRef(active)
  activeRef.current = active
  const emit = useRef<((name: string, payload: JsonValue) => void) | null>(null)
  const frameRef = useRef<HTMLIFrameElement>(null)
  const [html, setHtml] = useState('')
  const [sourceUrl, setSourceUrl] = useState('')
  const [ready, setReady] = useState(false)
  const [error, setError] = useState('')
  const [size, setSize] = useState({ width: 560, height: 560 })
  const taskIds = useRef(new Set<string>())
  const htmlCache = useRef(new MemoryCache<string>(16, 30_000))
  const urlCache = useRef(new MemoryCache<string>(16, 30_000))
  const dispose = useRef<null | (() => void)>(null)
  const loadPanel = (
    pluginId: string,
    panelId: string,
  ): Promise<{ url?: string; html?: string }> => {
    const key = `${pluginId}:${panelId}`
    // A srcDoc document inherits the app shell CSP, which blocks inline plugin
    // bundles in packaged builds. Desktop loads the same HTML from the asset
    // protocol with the plugin document's own policy.
    if (native) {
      return urlCache.current
        .get(key, () => call('plugin_panel_asset', { pluginId, panelId }).then(convertFileSrc))
        .then((url) => ({ url }))
    }
    return htmlCache.current
      .get(key, () => call('plugin_panel_html', { pluginId, panelId }))
      .then((html) => ({ html }))
  }
  useEffect(() => {
    let live = true
    const req = request
    setSize(initialPanelSize(req.size, { width: window.innerWidth, height: window.innerHeight }))
    void (
      req.assetPath && native
        ? Promise.resolve({ url: convertFileSrc(req.assetPath), html: undefined })
        : loadPanel(req.pluginId, req.panelId)
    )
      .then((source) => {
        if (!live) return
        if (source.url) setSourceUrl(source.url)
        if (source.html) setHtml(source.html)
      })
      .catch((cause) => {
        if (live) {
          setError(String(cause))
          reportPluginError(req.pluginId, 'panel', String(cause))
        }
      })
    return () => {
      live = false
      dispose.current?.()
      for (const id of taskIds.current) finishActivity(id, t('插件面板已释放'))
    }
  }, [])
  useEffect(() => {
    emit.current?.('panel.visibility', active)
  }, [active])
  useEffect(() => {
    if (error) {
      setReady(true)
      onReveal()
    }
  }, [error, onReveal])
  const attach = () => {
    if (!frameRef.current) return
    const target = frameRef.current
    dispose.current?.()
    const current = request
    const resources = new PanelResources(current.pluginId)
    const connect = () => {
      try {
        dispose.current = attachFiexPanel({
          getLocale,
          subscribeLocale,
          pluginId: current.pluginId,
          sessionId: crypto.randomUUID(),
          origin: 'null',
          frame: target,
          allowedMethods: new Set([
            'context.get',
            'clipboard.writeText',
            'plugin.invoke',
            'plugin.task',
            'panel.close',
            'panel.resize',
            'plugin.error',
            'theme.get',
            'dialog.openFile',
            'dialog.openDirectory',
            'dialog.saveFile',
            'dialog.openFiles',
            'navigation.open',
            'tasks.create',
            'tasks.spinner',
            'tasks.update',
            'tasks.finish',
            'tasks.cancel',
            'tasks.list',
          ]),
          onReady: () => {
            setReady(true)
            emit.current?.('theme.changed', themeSnapshot())
            emit.current?.('panel.visibility', activeRef.current)
            onRevealFallback()
          },
          onError: (message) => {
            setError(message)
            reportPluginError(current.pluginId, 'bridge', message)
          },
          subscribe: (send) => {
            emit.current = send
            const changed = () => send('theme.changed', themeSnapshot())
            window.addEventListener('fiex:theme-changed', changed)
            return () => {
              emit.current = null
              window.removeEventListener('fiex:theme-changed', changed)
            }
          },
          dispatch: createPanelDispatcher({
            current,
            getActive: () => activeRef.current,
            resources,
            taskIds: taskIds.current,
            onClose,
            onSettleReveal,
            setSize,
          }),
        })
      } catch (cause) {
        setError(String(cause))
        reportPluginError(current.pluginId, 'bridge', String(cause))
      }
    }
    connect()
  }
  return { frameRef, html, sourceUrl, ready, error, size, attach }
}

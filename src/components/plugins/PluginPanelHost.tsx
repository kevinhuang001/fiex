import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { PluginGlyph } from './PluginGlyph'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import { call, native } from '@/lib/api'
import { usePluginCatalog } from '@/lib/plugins'
import {
  beginActivity,
  cancelActivity,
  finishActivity,
  listActivity,
  updateActivity,
} from '@/hooks/useActivity'
import { attachFiexPanel } from '../../../sdk/ui/src/host'
import type { JsonValue } from '../../../sdk/ui/src/index'

export interface PluginPanelRequest {
  pluginId: string
  panelId: string
  title: string
  selection: string[]
  directory: string
  icon?: string
  anchor?: { x: number; y: number }
  source?: 'toolbar' | 'contextMenu'
  size?: { width: number; height: number }
}
export const OPEN_PLUGIN_PANEL = 'fiex:plugin-panel:open'
export const PLUGIN_RESULT_EVENT = 'fiex:plugin-result'

const PANEL_CSP =
  "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'\">"

/** Plugin iframe is sandboxed to an opaque origin. Its only way to reach Core
 * is the per-panel MessageChannel, never Tauri's global invoke. */
export function PluginPanelHost() {
  const [request, setRequest] = useState<PluginPanelRequest | null>(null)
  const [html, setHtml] = useState('')
  const [sourceUrl, setSourceUrl] = useState('')
  const [ready, setReady] = useState(false)
  const [error, setError] = useState('')
  const [result, setResult] = useState<{ title: string; value: unknown } | null>(null)
  const [size, setSize] = useState({ width: 560, height: 560 })
  const panel = useRef<HTMLDivElement>(null)
  const frame = useRef<HTMLIFrameElement>(null)
  const bridgeReady = useRef(false)
  const retryTimers = useRef<number[]>([])
  const readyTimer = useRef<number | null>(null)
  const taskIds = useRef(new Set<string>())
  const htmlCache = useRef(new Map<string, string>())
  const urlCache = useRef(new Map<string, string>())
  const plugins = usePluginCatalog()
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
      const cachedUrl = urlCache.current.get(key)
      if (cachedUrl) return Promise.resolve({ url: cachedUrl })
      return call<string>('plugin_panel_asset', { pluginId, panelId }).then((path) => {
        const url = convertFileSrc(path)
        urlCache.current.set(key, url)
        return { url }
      })
    }
    const cached = htmlCache.current.get(key)
    if (cached) return Promise.resolve({ html: cached })
    return call<string>('plugin_panel_html', { pluginId, panelId }).then((value) => {
      htmlCache.current.set(key, value)
      return { html: value }
    })
  }
  useEffect(() => {
    const onOpen = (event: Event) => {
      const req = (event as CustomEvent<PluginPanelRequest>).detail
      if (!req?.pluginId || !req.panelId) return
      dispose.current?.()
      dispose.current = null
      if (req.directory) {
        void call('plugin_set_active_directory', { path: req.directory }).catch(() => {})
      }
      setRequest(req)
      setReady(false)
      setHtml(htmlCache.current.get(`${req.pluginId}:${req.panelId}`) || '')
      setSourceUrl(urlCache.current.get(`${req.pluginId}:${req.panelId}`) || '')
      setError('')
      const nextSize = req.size || { width: 560, height: 560 }
      setSize({
        width: Math.min(nextSize.width, window.innerWidth - 40),
        height: Math.min(nextSize.height, window.innerHeight - 92),
      })
      void loadPanel(req.pluginId, req.panelId)
        .then((source) => {
          if (source.url) setSourceUrl(source.url)
          if (source.html) setHtml(source.html)
        })
        .catch((cause) => setError(String(cause)))
    }
    const onResult = (event: Event) => {
      const detail = (event as CustomEvent<{ title: string; result: unknown }>).detail
      setResult({ title: detail.title, value: detail.result })
    }
    window.addEventListener(PLUGIN_RESULT_EVENT, onResult)
    window.addEventListener(OPEN_PLUGIN_PANEL, onOpen)
    return () => {
      window.removeEventListener(OPEN_PLUGIN_PANEL, onOpen)
      window.removeEventListener(PLUGIN_RESULT_EVENT, onResult)
      dispose.current?.()
    }
  }, [])
  useEffect(() => {
    if (!request) return
    const outside = (event: PointerEvent) => {
      if (!panel.current?.contains(event.target as Node)) close()
    }
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close()
    }
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape)
    return () => {
      document.removeEventListener('pointerdown', outside)
      document.removeEventListener('keydown', escape)
    }
  })
  useEffect(() => {
    if (error) setReady(true)
  }, [error])
  const close = () => {
    if (readyTimer.current !== null) window.clearTimeout(readyTimer.current)
    readyTimer.current = null
    retryTimers.current.forEach((timer) => window.clearTimeout(timer))
    retryTimers.current = []
    for (const id of taskIds.current) finishActivity(id, '插件面板已关闭')
    taskIds.current.clear()
    dispose.current?.()
    dispose.current = null
    setRequest(null)
    setReady(false)
    setHtml('')
    setSourceUrl('')
  }
  const attach = () => {
    if (!frame.current || !request) return
    const target = frame.current
    dispose.current?.()
    retryTimers.current.forEach((timer) => window.clearTimeout(timer))
    retryTimers.current = []
    bridgeReady.current = false
    const current = request
    const connect = () => {
      try {
        dispose.current = attachFiexPanel({
          pluginId: current.pluginId,
          sessionId: crypto.randomUUID(),
          origin: 'null',
          frame: target,
          allowedMethods: new Set([
            'context.get',
            'clipboard.writeText',
            'plugin.invoke',
            'panel.close',
            'panel.resize',
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
            bridgeReady.current = true
            retryTimers.current.forEach((timer) => window.clearTimeout(timer))
            retryTimers.current = []
            if (readyTimer.current !== null) window.clearTimeout(readyTimer.current)
            readyTimer.current = window.setTimeout(() => setReady(true), 40)
          },
          dispatch: async (method, params): Promise<JsonValue> => {
            if (method === 'context.get')
              return {
                directory: current.directory,
                selection: current.selection,
                directoryPath: current.directory,
                selectionPaths: current.selection,
              }
            if (method === 'panel.close') {
              close()
              return null
            }
            if (method === 'panel.resize') {
              const p = (params || {}) as { width?: number; height?: number }
              if (Number.isFinite(p.width) && Number.isFinite(p.height)) {
                setSize({
                  width: Math.min(window.innerWidth - 40, Math.max(320, Number(p.width))),
                  height: Math.min(window.innerHeight - 92, Math.max(220, Number(p.height))),
                })
              }
              return null
            }
            if (method === 'theme.get') return document.documentElement.dataset.theme || 'system'
            if (method === 'navigation.open') {
              const p = params as { path?: unknown } | null
              if (!p || typeof p.path !== 'string' || !p.path)
                throw new Error('Invalid navigation path')
              window.dispatchEvent(
                new CustomEvent('fiex:navigate', { detail: { path: p.path, pane: 'left' } }),
              )
              return null
            }
            if (method === 'tasks.spinner') {
              const p = (params || {}) as {
                title?: unknown
                detail?: unknown
              }
              if (typeof p.title !== 'string' || !p.title.trim())
                throw new Error('Invalid task title')
              const id = beginActivity(
                p.title.trim(),
                typeof p.detail === 'string' ? p.detail : undefined,
                undefined,
                'plugin',
              )
              taskIds.current.add(id)
              updateActivity(id, { indeterminate: true })
              return { taskId: id }
            }
            if (method === 'tasks.create') {
              const p = (params || {}) as {
                title?: unknown
                detail?: unknown
                progress?: unknown
                indeterminate?: unknown
              }
              if (typeof p.title !== 'string' || !p.title.trim())
                throw new Error('Invalid task title')
              const id = beginActivity(
                p.title.trim(),
                typeof p.detail === 'string' ? p.detail : undefined,
                undefined,
                'plugin',
              )
              taskIds.current.add(id)
              if (typeof p.progress === 'number') updateActivity(id, { progress: p.progress })
              if (p.indeterminate === true) updateActivity(id, { indeterminate: true })
              return { taskId: id }
            }
            if (method === 'tasks.update') {
              const p = (params || {}) as {
                taskId?: unknown
                label?: unknown
                detail?: unknown
                progress?: unknown
                indeterminate?: unknown
                done?: unknown
                total?: unknown
                step?: unknown
              }
              if (typeof p.taskId !== 'string' || !taskIds.current.has(p.taskId))
                throw new Error('Unknown task')
              updateActivity(p.taskId, {
                ...(typeof p.label === 'string' ? { label: p.label } : {}),
                ...(typeof p.detail === 'string' ? { detail: p.detail } : {}),
                ...(typeof p.progress === 'number' ? { progress: p.progress } : {}),
                ...(typeof p.indeterminate === 'boolean' ? { indeterminate: p.indeterminate } : {}),
                ...(typeof p.done === 'number' ? { done: p.done } : {}),
                ...(typeof p.total === 'number' ? { total: p.total } : {}),
                ...(typeof p.step === 'string' ? { step: p.step } : {}),
              })
              return null
            }
            if (method === 'tasks.finish') {
              const p = (params || {}) as { taskId?: unknown; error?: unknown; detail?: unknown }
              if (typeof p.taskId !== 'string' || !taskIds.current.has(p.taskId))
                throw new Error('Unknown task')
              finishActivity(
                p.taskId,
                typeof p.error === 'string' ? p.error : undefined,
                typeof p.detail === 'string' ? p.detail : undefined,
              )
              taskIds.current.delete(p.taskId)
              return null
            }
            if (method === 'tasks.cancel') {
              const p = (params || {}) as { taskId?: unknown }
              if (typeof p.taskId !== 'string' || !taskIds.current.has(p.taskId))
                throw new Error('Unknown task')
              cancelActivity(p.taskId)
              taskIds.current.delete(p.taskId)
              return null
            }
            if (method === 'tasks.list')
              return listActivity()
                .filter((item) => item.kind === 'plugin')
                .map((item) => ({ ...item }))
            if (method === 'clipboard.writeText') {
              const text = (params as { text?: unknown })?.text
              if (typeof text !== 'string' || text.length > 1024 * 1024)
                throw new Error('Invalid clipboard text')
              await navigator.clipboard.writeText(text)
              return null
            }
            if (method === 'plugin.invoke') {
              const p = params as { method?: unknown; params?: unknown } | null
              if (!p || typeof p.method !== 'string' || !p.method || p.method.startsWith('host.'))
                throw new Error('Invalid plugin method')
              const networkDomains = await call<string[]>('plugin_network_domains', {
                pluginId: current.pluginId,
              }).catch(() => [])
              const baseParams =
                p.params && typeof p.params === 'object' && !Array.isArray(p.params)
                  ? { ...(p.params as Record<string, unknown>) }
                  : {}
              return await call<JsonValue>('plugin_invoke', {
                pluginId: current.pluginId,
                method: p.method,
                params: { ...baseParams, networkDomains },
                directory: current.directory,
              })
            }
            if (method.startsWith('dialog.')) {
              // The chooser is rendered by the host and only returns the path the
              // user explicitly picks; it never grants directory browsing.
              const p = (params || {}) as {
                directory?: string
                suggestedName?: string
                extensions?: string[]
              }
              if (method === 'dialog.openFiles')
                return await call<string[] | null>('plugin_files_dialog', {
                  directory: p.directory,
                  extensions: p.extensions,
                })
              return await call<string | null>('plugin_file_dialog', {
                mode: method.slice('dialog.'.length),
                directory: p.directory,
                suggestedName: p.suggestedName,
                extensions: p.extensions,
              })
            }
            throw new Error('Unsupported panel operation')
          },
        })
      } catch (cause) {
        setError(String(cause))
      }
    }
    connect()
    // Module scripts may execute after iframe `load`; the initial port can be
    // posted before the client is listening. Retry with a fresh channel until
    // the ready handshake arrives.
    retryTimers.current = [80, 300, 1000].map((delay) =>
      window.setTimeout(() => {
        if (!bridgeReady.current) {
          dispose.current?.()
          connect()
        }
      }, delay),
    )
  }
  useEffect(() => {
    if (!import.meta.env.DEV) return
    for (const plugin of plugins) {
      for (const tool of plugin.contributions?.toolbar || []) {
        if (tool.panel) void loadPanel(plugin.id, tool.id).catch(() => {})
      }
      for (const item of plugin.contributions?.contextMenus || []) {
        if (item.panel) void loadPanel(plugin.id, item.id).catch(() => {})
        for (const child of item.children || []) {
          if (child.panel) void loadPanel(plugin.id, child.id).catch(() => {})
        }
      }
    }
  }, [plugins])
  const panelWidth = Math.min(size.width, window.innerWidth - 40)
  return (
    <>
      {request && (
        <div
          ref={panel}
          className={`plugin-panel-popover${ready ? ' ready' : ' opening'}${
            request.source === 'toolbar' ? ' toolbar-anchored' : ''
          }`}
          role="dialog"
          aria-label={request.title || '插件'}
          aria-busy={!ready}
          style={
            request.source === 'toolbar'
              ? {
                  width: panelWidth,
                  height: Math.min(size.height + 52, window.innerHeight - 96 - 16),
                  top: 96,
                  right: Math.max(
                    18,
                    Math.min(
                      window.innerWidth - (request.anchor?.x ?? window.innerWidth - 18),
                      window.innerWidth - panelWidth - 18,
                    ),
                  ),
                }
              : {
                  width: panelWidth,
                  height: Math.min(size.height + 52, window.innerHeight - 40),
                }
          }
        >
          <header>
            <span className="plugin-panel-icon">
              <PluginGlyph icon={request.icon} />
            </span>
            <strong>{request.title || '插件'}</strong>
            <button
              className="icon-button plugin-panel-close"
              aria-label="关闭插件面板"
              onClick={close}
            >
              <X size={17} />
            </button>
          </header>
          <div className="plugin-panel-content">
            {error ? (
              <p role="alert" className="connection-error">
                {error}
              </p>
            ) : sourceUrl ? (
              <iframe
                key="asset"
                ref={frame}
                title={request.title || '插件界面'}
                sandbox="allow-scripts"
                referrerPolicy="no-referrer"
                style={{ height: '100%', width: '100%', border: 0 }}
                src={sourceUrl}
                onLoad={() => {
                  attach()
                  if (readyTimer.current !== null) window.clearTimeout(readyTimer.current)
                  readyTimer.current = window.setTimeout(() => setReady(true), 700)
                }}
              />
            ) : html ? (
              <iframe
                key="srcdoc"
                ref={frame}
                title={request.title || '插件界面'}
                sandbox="allow-scripts"
                referrerPolicy="no-referrer"
                style={{ height: '100%', width: '100%', border: 0 }}
                srcDoc={PANEL_CSP + html}
                onLoad={() => {
                  attach()
                  if (readyTimer.current !== null) window.clearTimeout(readyTimer.current)
                  readyTimer.current = window.setTimeout(() => setReady(true), 700)
                }}
              />
            ) : null}
          </div>
        </div>
      )}
      <Modal
        open={Boolean(result)}
        title={result?.title || '插件结果'}
        onClose={() => setResult(null)}
        wide
      >
        <pre className="plugin-command-result">
          {result && JSON.stringify(result.value, null, 2)}
        </pre>
        <footer className="plugin-consent-actions">
          <Button onClick={() => setResult(null)}>关闭</Button>
        </footer>
      </Modal>
    </>
  )
}

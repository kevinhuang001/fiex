import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import { PluginGlyph } from './PluginGlyph'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import { call } from '@/lib/api'
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
  const [error, setError] = useState('')
  const [result, setResult] = useState<{ title: string; value: unknown } | null>(null)
  const [size, setSize] = useState({ width: 440, height: 520 })
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null)
  const [position, setPosition] = useState({ left: 12, top: 96 })
  const pointer = useRef<{ x: number; y: number } | null>(null)
  const panel = useRef<HTMLDivElement>(null)
  const frame = useRef<HTMLIFrameElement>(null)
  const bridgeReady = useRef(false)
  const retryTimers = useRef<number[]>([])
  const taskIds = useRef(new Set<string>())
  const dispose = useRef<null | (() => void)>(null)
  useEffect(() => {
    const onOpen = (event: Event) => {
      const req = (event as CustomEvent<PluginPanelRequest>).detail
      if (!req?.pluginId || !req.panelId) return
      dispose.current?.()
      dispose.current = null
      setRequest(req)
      setHtml('')
      setError('')
      const point = req.anchor || pointer.current
      setAnchor(point)
      const nextSize = { width: 440, height: Math.min(520, window.innerHeight - 120) }
      setSize(nextSize)
      setPosition(place(point, nextSize))
      void call<string>('plugin_panel_html', { pluginId: req.pluginId, panelId: req.panelId })
        .then(setHtml)
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
  const place = (point: { x: number; y: number } | null, box = size) => {
    const width = Math.min(box.width, window.innerWidth - 24)
    const height = Math.min(window.innerHeight * 0.66, 600, window.innerHeight - 24)
    const x = point?.x ?? window.innerWidth - 18
    // Horizontal placement follows the pointer; vertical placement/height
    // matches the Activity Center instead of following the pointer.
    return {
      left: Math.max(12, Math.min(x + 10, window.innerWidth - width - 12)),
      top: Math.max(12, Math.min(96, window.innerHeight - height - 12)),
    }
  }
  useEffect(() => {
    const track = (event: PointerEvent) => {
      pointer.current = { x: event.clientX, y: event.clientY }
    }
    document.addEventListener('pointerdown', track, true)
    return () => document.removeEventListener('pointerdown', track, true)
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
    if (!request) return
    const reposition = () => setPosition(place(anchor, size))
    reposition()
    window.addEventListener('resize', reposition)
    return () => window.removeEventListener('resize', reposition)
  }, [request, anchor, size])
  const close = () => {
    retryTimers.current.forEach((timer) => window.clearTimeout(timer))
    retryTimers.current = []
    for (const id of taskIds.current) finishActivity(id, '插件面板已关闭')
    taskIds.current.clear()
    dispose.current?.()
    dispose.current = null
    setRequest(null)
    setHtml('')
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
            'tasks.update',
            'tasks.finish',
            'tasks.cancel',
            'tasks.list',
          ]),
          onReady: () => {
            bridgeReady.current = true
            retryTimers.current.forEach((timer) => window.clearTimeout(timer))
            retryTimers.current = []
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
                setSize((old) => ({
                  ...old,
                  width: Math.min(1000, Math.max(360, Number(p.width))),
                }))
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
            if (method === 'tasks.create') {
              const p = (params || {}) as {
                title?: unknown
                detail?: unknown
                progress?: unknown
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
              return { taskId: id }
            }
            if (method === 'tasks.update') {
              const p = (params || {}) as {
                taskId?: unknown
                label?: unknown
                detail?: unknown
                progress?: unknown
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
  return (
    <>
      {request && (
        <div
          ref={panel}
          className="plugin-panel-popover"
          role="dialog"
          aria-label={request.title || '插件'}
          style={{
            width: Math.min(size.width, window.innerWidth - 24),
            height: Math.min(
              size.height + 54,
              window.innerHeight * 0.66,
              600,
              window.innerHeight - 24,
            ),
            left: position.left,
            top: position.top,
          }}
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
            ) : !html ? (
              <p className="sidebar-hint">正在打开插件界面…</p>
            ) : (
              <iframe
                ref={frame}
                title={request.title || '插件界面'}
                sandbox="allow-scripts"
                referrerPolicy="no-referrer"
                style={{ height: '100%', width: '100%', border: 0 }}
                srcDoc={PANEL_CSP + html}
                onLoad={attach}
              />
            )}
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

import { DemoFilePicker, type FakePicker } from './DemoFilePicker'
import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { PluginGlyph } from './PluginGlyph'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import { themeSnapshot } from '@/lib/themes'
import { call, native } from '@/lib/api'
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

const PANEL_EXIT_MS = 90
const PANEL_REVEAL_FALLBACK_MS = 260
const MAX_PANEL_WIDTH = 720
const MAX_PANEL_HEIGHT = 680

const PANEL_CSP =
  "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'\">"

/** Plugin iframe is sandboxed to an opaque origin. Its only way to reach Core
 * is the per-panel MessageChannel, never Tauri's global invoke. */
export function PluginPanelHost() {
  const [panels, setPanels] = useState<PluginPanelRequest[]>([])
  const [active, setActive] = useState('')
  // The outgoing panel animates out before the next one mounts, so switching
  // between toolbar tools never shows two panels at once. At most one swap is
  // in flight; a newer click replaces the queued request.
  const [leaving, setLeaving] = useState('')
  const leaveTimer = useRef<number | null>(null)
  const activeKey = useRef('')
  const pending = useRef<{ request: PluginPanelRequest; key: string } | null>(null)
  // A toolbar click outside the panel closes it on pointerdown, one event
  // before the new panel opens; remember it briefly so the swap still animates.
  const closedKey = useRef('')
  const closedAt = useRef(0)
  activeKey.current = active
  const keyOf = (req: PluginPanelRequest) =>
    JSON.stringify([req.pluginId, req.panelId, req.directory, req.selection])
  const cancelExit = () => {
    if (leaveTimer.current !== null) {
      window.clearTimeout(leaveTimer.current)
      leaveTimer.current = null
    }
    pending.current = null
    setLeaving('')
  }
  useEffect(() => {
    const show = (request: PluginPanelRequest, key: string) => {
      setPanels((items) => [...items.filter((item) => keyOf(item) !== key), request].slice(-3))
      setActive(key)
    }
    const open = (event: Event) => {
      const req = (event as CustomEvent<PluginPanelRequest>).detail
      if (!req?.pluginId || !req.panelId) return
      const key = keyOf(req)
      const previous =
        activeKey.current || (performance.now() - closedAt.current < 150 ? closedKey.current : '')
      if (!previous || previous === key) {
        cancelExit()
        show(req, key)
        return
      }
      pending.current = { request: req, key }
      setLeaving(previous)
      if (leaveTimer.current !== null) window.clearTimeout(leaveTimer.current)
      leaveTimer.current = window.setTimeout(() => {
        leaveTimer.current = null
        const next = pending.current
        pending.current = null
        setLeaving('')
        if (next) show(next.request, next.key)
      }, PANEL_EXIT_MS)
    }
    const invalidate = () => {
      cancelExit()
      setPanels([])
      setActive('')
    }
    window.addEventListener(OPEN_PLUGIN_PANEL, open)
    window.addEventListener('fiex:plugins-changed', invalidate)
    return () => {
      window.removeEventListener(OPEN_PLUGIN_PANEL, open)
      window.removeEventListener('fiex:plugins-changed', invalidate)
      if (leaveTimer.current !== null) window.clearTimeout(leaveTimer.current)
    }
  }, [])
  return (
    <>
      {panels.map((request) => {
        const key = keyOf(request)
        const exiting = leaving === key
        return (
          <CachedPanel
            key={key}
            initialRequest={request}
            active={active === key && !exiting}
            leaving={exiting}
            onClose={() => {
              closedKey.current = key
              closedAt.current = performance.now()
              cancelExit()
              setActive('')
            }}
          />
        )
      })}
      <PluginResult />
    </>
  )
}
function PluginResult() {
  const [result, setResult] = useState<{ title: string; value: unknown } | null>(null)
  useEffect(() => {
    const show = (event: Event) => {
      const detail = (event as CustomEvent<{ title: string; result: unknown }>).detail
      setResult({ title: detail.title, value: detail.result })
    }
    window.addEventListener(PLUGIN_RESULT_EVENT, show)
    return () => window.removeEventListener(PLUGIN_RESULT_EVENT, show)
  }, [])
  return (
    <Modal
      open={Boolean(result)}
      title={result?.title || '插件结果'}
      onClose={() => setResult(null)}
      wide
    >
      <pre className="plugin-command-result">{result && JSON.stringify(result.value, null, 2)}</pre>
      <footer className="plugin-consent-actions">
        <Button onClick={() => setResult(null)}>关闭</Button>
      </footer>
    </Modal>
  )
}
function CachedPanel({
  initialRequest,
  active,
  leaving,
  onClose,
}: {
  initialRequest: PluginPanelRequest
  active: boolean
  leaving: boolean
  onClose: () => void
}) {
  const request = initialRequest
  const activeRef = useRef(active)
  activeRef.current = active
  const emit = useRef<((name: string, payload: JsonValue) => void) | null>(null)
  const [html, setHtml] = useState('')
  const [sourceUrl, setSourceUrl] = useState('')
  const [ready, setReady] = useState(false)
  // The sheet stays invisible until the iframe has finished its own layout and
  // stopped asking for new sizes, so it never appears at a size it is about to
  // leave (that read as a jump the moment the panel opened).
  const [revealed, setRevealed] = useState(false)
  const revealTimer = useRef<number | null>(null)
  const [error, setError] = useState('')
  const [fakePicker, setFakePicker] = useState<FakePicker | null>(null)
  const [size, setSize] = useState({ width: 560, height: 560 })
  const panel = useRef<HTMLDivElement>(null)
  const frame = useRef<HTMLIFrameElement>(null)
  const taskIds = useRef(new Set<string>())
  const htmlCache = useRef(new Map<string, string>())
  const urlCache = useRef(new Map<string, string>())
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
    let live = true
    const req = initialRequest
    const nextSize = req.size || { width: 560, height: 560 }
    setSize({
      width: Math.min(MAX_PANEL_WIDTH, nextSize.width, window.innerWidth - 40),
      height: Math.min(MAX_PANEL_HEIGHT, nextSize.height, window.innerHeight - 92),
    })
    void loadPanel(req.pluginId, req.panelId)
      .then((source) => {
        if (!live) return
        if (source.url) setSourceUrl(source.url)
        if (source.html) setHtml(source.html)
      })
      .catch((cause) => {
        if (live) setError(String(cause))
      })
    return () => {
      live = false
      dispose.current?.()
      for (const id of taskIds.current) finishActivity(id, '插件面板已释放')
    }
  }, [])
  useEffect(() => {
    emit.current?.('panel.visibility', active)
  }, [active])
  useEffect(() => {
    if (!active) return
    const outside = (event: PointerEvent) => {
      if (fakePicker) return
      if (!panel.current?.contains(event.target as Node)) close()
    }
    const escape = (event: KeyboardEvent) => {
      if (fakePicker) return
      if (event.key === 'Escape') close()
    }
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape)
    return () => {
      document.removeEventListener('pointerdown', outside)
      document.removeEventListener('keydown', escape)
    }
  })
  const reveal = () => {
    if (revealTimer.current !== null) {
      window.clearTimeout(revealTimer.current)
      revealTimer.current = null
    }
    setRevealed(true)
  }
  // Panels measure themselves and call panel.resize; a quiet stretch after the
  // last request is the signal that the layout has settled.
  const settleReveal = () => {
    if (revealTimer.current !== null) window.clearTimeout(revealTimer.current)
    revealTimer.current = window.setTimeout(() => {
      revealTimer.current = null
      setRevealed(true)
    }, 120)
  }
  useEffect(() => {
    if (error) {
      setReady(true)
      reveal()
    }
  }, [error])
  useEffect(
    () => () => {
      if (revealTimer.current !== null) window.clearTimeout(revealTimer.current)
    },
    [],
  )
  const close = () => {
    onClose()
  }
  const attach = () => {
    if (!frame.current) return
    const target = frame.current
    dispose.current?.()
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
            setReady(true)
            emit.current?.('theme.changed', themeSnapshot())
            emit.current?.('panel.visibility', activeRef.current)
            // Fallback for panels that never resize themselves; resize-driven
            // panels reveal sooner, as soon as their layout goes quiet.
            revealTimer.current = window.setTimeout(() => {
              revealTimer.current = null
              setRevealed(true)
            }, PANEL_REVEAL_FALLBACK_MS)
          },
          onError: setError,
          subscribe: (send) => {
            emit.current = send
            const changed = () => send('theme.changed', themeSnapshot())
            window.addEventListener('fiex:theme-changed', changed)
            return () => {
              emit.current = null
              window.removeEventListener('fiex:theme-changed', changed)
            }
          },
          dispatch: async (method, params): Promise<JsonValue> => {
            if (
              !activeRef.current &&
              !['theme.get', 'context.get', 'tasks.update', 'tasks.finish'].includes(method)
            )
              throw new Error('Panel is hidden')
            if (method === 'context.get')
              return {
                directoryPath: current.directory,
                selectionPaths: current.selection,
                panelId: current.panelId,
                panelTitle: current.title,
                ...(current.source ? { source: current.source } : {}),
              }
            if (method === 'panel.close') {
              close()
              return null
            }
            if (method === 'panel.resize') {
              const p = (params || {}) as { width?: number; height?: number }
              if (Number.isFinite(p.width) && Number.isFinite(p.height)) {
                setSize({
                  width: Math.min(
                    MAX_PANEL_WIDTH,
                    window.innerWidth - 40,
                    Math.max(320, Number(p.width)),
                  ),
                  height: Math.min(
                    MAX_PANEL_HEIGHT,
                    window.innerHeight - 92,
                    Math.max(220, Number(p.height)),
                  ),
                })
                settleReveal()
              }
              return null
            }
            if (method === 'theme.get') return themeSnapshot()
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
              const baseParams =
                p.params && typeof p.params === 'object' && !Array.isArray(p.params)
                  ? { ...(p.params as Record<string, unknown>) }
                  : {}
              return await call<JsonValue>('plugin_invoke', {
                pluginId: current.pluginId,
                method: p.method,
                params: baseParams,
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
              if (!native) {
                const picked = await new Promise<string | string[] | null>((resolve) => {
                  setFakePicker({
                    mode: method,
                    suggestedName: p.suggestedName,
                    directory: p.directory || current.directory,
                    resolve,
                  })
                })
                if (method === 'dialog.openFiles')
                  return Array.isArray(picked) ? picked : picked ? [picked] : null
                return typeof picked === 'string' ? picked : null
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
  }
  const panelWidth = Math.min(MAX_PANEL_WIDTH, size.width, window.innerWidth - 40)
  return (
    <>
      {request && (
        <div
          ref={panel}
          className={`plugin-panel-popover${
            request.source === 'toolbar' ? ' toolbar-anchored' : ''
          }${revealed ? ' revealed' : ''}${leaving ? ' leaving' : ''}`}
          hidden={!active && !leaving}
          role="dialog"
          aria-label={request.title || '插件'}
          aria-busy={!ready}
          style={
            request.source === 'toolbar'
              ? {
                  width: panelWidth,
                  height: Math.min(
                    size.height + 52,
                    MAX_PANEL_HEIGHT + 52,
                    window.innerHeight - 96 - 16,
                  ),
                  top: 96,
                  left: Math.max(
                    12,
                    Math.min(
                      (request.anchor?.x ?? window.innerWidth - 18) - panelWidth / 2,
                      window.innerWidth - panelWidth - 12,
                    ),
                  ),
                  right: 'auto',
                }
              : {
                  width: panelWidth,
                  height: Math.min(
                    size.height + 52,
                    MAX_PANEL_HEIGHT + 52,
                    window.innerHeight - 40,
                  ),
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
            {!ready && !error && (
              <div role="status" className="plugin-panel-loading">
                正在加载插件…
              </div>
            )}
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
                }}
              />
            ) : null}
          </div>
        </div>
      )}
      {fakePicker && <DemoFilePicker fakePicker={fakePicker} onClose={() => setFakePicker(null)} />}
    </>
  )
}

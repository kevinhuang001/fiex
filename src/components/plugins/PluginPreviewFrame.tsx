import { getLocale, subscribeLocale } from '@/i18n'
import { useEffect, useRef, useState } from 'react'
import { call } from '@/lib/api'
import { themeSnapshot } from '@/lib/themes'
import { attachFiexPanel } from '../../../sdk/ui/src/host'
import type { JsonValue } from '../../../sdk/ui/src/index'

/** Standard plugin preview document returned by Core's PreviewView. The
 * iframe only sees the opaque session and projected resource context. A plugin
 * may also declare a package-local loading document; it runs in the same
 * sandboxed Core-owned iframe but without a Core preview session. */
export function PluginPreviewFrame({
  pluginId,
  contributionId,
  html,
  src,
  context,
  sessionId,
  timeoutMs,
  loading = false,
  onPreviewReady,
  name,
  extension,
}: {
  pluginId: string
  contributionId: string
  html?: string
  /** Core-cached asset URL for a desktop loading document. */
  src?: string
  context?: unknown
  sessionId: string
  /** Manifest-negotiated budget; Core defaults when the plugin omits it. */
  timeoutMs?: number
  /** Loading documents may only read context/theme and report errors. */
  loading?: boolean
  /** Called once the plugin's preview data has rendered in the frame. */
  onPreviewReady?: () => void
  name: string
  extension: string
}) {
  const frame = useRef<HTMLIFrameElement>(null)
  const dispose = useRef<(() => void) | null>(null)
  const emit = useRef<((name: string, payload: JsonValue) => void) | null>(null)
  const attachRef = useRef<(() => void) | null>(null)
  const [error, setError] = useState('')
  const previewReady = useRef(onPreviewReady)
  previewReady.current = onPreviewReady

  useEffect(() => {
    const element = frame.current
    if (!element || !sessionId) return
    let attached = false
    const attach = () => {
      if (attached) return
      attached = true
      dispose.current = attachFiexPanel({
        getLocale,
        subscribeLocale,
        pluginId,
        sessionId,
        origin: 'null',
        frame: element,
        allowedMethods: loading
          ? new Set(['context.get', 'theme.get', 'plugin.error'])
          : new Set([
              'context.get',
              'theme.get',
              'plugin.invoke',
              'plugin.task',
              'plugin.error',
              'preview.ready',
              'clipboard.writeText',
              'navigation.open',
              'panel.close',
              'panel.resize',
            ]),
        onReady: () => emit.current?.('theme.changed', themeSnapshot()),
        onError: (value) => {
          // A static loading page may never open the bridge; keep it visible
          // until preview.open replaces it.
          if (!loading) setError(value)
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
        dispatch: async (method, params): Promise<JsonValue> => {
          if (method === 'context.get') {
            const settings = await call<Record<string, JsonValue>>('plugin_settings', {
              pluginId,
            }).catch(() => ({}))
            return {
              ...(loading ? {} : ((context || {}) as Record<string, JsonValue>)),
              // Compatibility alias for panel code that only needs the name.
              preview: { name, extension },
              locale: getLocale(),
              uiTimeoutMs: timeoutMs ?? 15000,
              settings,
            } as JsonValue
          }
          if (method === 'theme.get') return themeSnapshot()
          if (method === 'preview.ready') {
            previewReady.current?.()
            return null
          }
          if (method === 'plugin.error') {
            const p = (params || {}) as { message?: unknown; detail?: unknown }
            if (typeof p.message !== 'string' || !p.message.trim())
              throw new Error('Invalid error message')
            await call('ui_error_report', {
              pluginId,
              source: 'ui',
              message: p.message.slice(0, 500),
              detail: typeof p.detail === 'string' ? p.detail.slice(0, 4000) : undefined,
            })
            return null
          }
          if (loading) throw new Error('Method not allowed')
          if (method === 'plugin.invoke') {
            const p = params as { method?: unknown; params?: unknown } | null
            if (!p || typeof p.method !== 'string' || !p.method || p.method.startsWith('host.'))
              throw new Error('Invalid plugin method')
            return call<JsonValue>('plugin_invoke', {
              pluginId,
              method: p.method,
              params: p.params ?? null,
              previewSessionId: sessionId,
            })
          }
          if (method === 'plugin.task') {
            const p = params as { taskId?: unknown } | null
            if (!p || typeof p.taskId !== 'string' || !p.taskId)
              throw new Error('Invalid plugin task id')
            return call<JsonValue | null>('plugin_task', {
              pluginId,
              taskId: p.taskId,
            })
          }
          if (method === 'panel.close' || method === 'panel.resize') return null
          if (method === 'clipboard.writeText') {
            const text = (params as { text?: unknown } | null)?.text
            if (typeof text !== 'string' || text.length > 1024 * 1024)
              throw new Error('Invalid clipboard text')
            await navigator.clipboard.writeText(text)
            return null
          }
          if (method === 'navigation.open') {
            const path = (params as { path?: unknown } | null)?.path
            if (typeof path !== 'string' || !path) throw new Error('Invalid navigation path')
            window.dispatchEvent(
              new CustomEvent('fiex:navigate', { detail: { path, pane: 'left' } }),
            )
            return null
          }
          throw new Error('Method not allowed')
        },
      })
    }
    attachRef.current = attach
    element.addEventListener('load', attach)
    if (element.contentDocument?.readyState === 'complete') attach()
    return () => {
      attachRef.current = null
      element.removeEventListener('load', attach)
      dispose.current?.()
      dispose.current = null
      emit.current = null
    }
  }, [html, src, sessionId, pluginId, timeoutMs, loading, name, extension, context])

  if (error) return <div className="preview-plugin-state">{error}</div>
  return (
    <iframe
      ref={frame}
      className="preview-plugin-frame"
      sandbox="allow-scripts"
      src={src}
      srcDoc={src ? undefined : html}
      title={name}
      data-contribution={contributionId}
      data-loading={loading ? 'true' : undefined}
      referrerPolicy="no-referrer"
      onLoad={() => attachRef.current?.()}
    />
  )
}

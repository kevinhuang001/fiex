import { useEffect, useRef, useState } from 'react'
import { call } from '@/lib/api'
import { themeSnapshot } from '@/lib/themes'
import { attachFiexPanel } from '../../../sdk/ui/src/host'
import type { JsonValue } from '../../../sdk/ui/src/index'

/** Standard plugin preview document returned by Core's PreviewView. The
 * iframe only sees the opaque session and projected resource context. */
export function PluginPreviewFrame({
  pluginId,
  contributionId,
  html,
  context,
  sessionId,
  name,
  extension,
}: {
  pluginId: string
  contributionId: string
  html: string
  context: unknown
  sessionId: string
  name: string
  extension: string
}) {
  const frame = useRef<HTMLIFrameElement>(null)
  const dispose = useRef<(() => void) | null>(null)
  const emit = useRef<((name: string, payload: JsonValue) => void) | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    const element = frame.current
    if (!element || !sessionId) return
    let attached = false
    const attach = () => {
      if (attached) return
      attached = true
      dispose.current = attachFiexPanel({
        pluginId,
        sessionId,
        origin: 'null',
        frame: element,
        allowedMethods: new Set([
          'context.get',
          'theme.get',
          'plugin.invoke',
          'plugin.error',
          'clipboard.writeText',
          'navigation.open',
          'panel.close',
          'panel.resize',
        ]),
        onReady: () => emit.current?.('theme.changed', themeSnapshot()),
        onError: (value) => setError(value),
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
            return {
              ...((context || {}) as Record<string, JsonValue>),
              // Compatibility alias for panel code that only needs the name.
              preview: { name, extension },
            } as JsonValue
          }
          if (method === 'theme.get') return themeSnapshot()
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
          if (method === 'plugin.error') {
            const p = (params || {}) as { message?: unknown; detail?: unknown }
            if (typeof p.message !== 'string' || !p.message.trim())
              throw new Error('Invalid error message')
            await call('plugin_error_report', {
              pluginId,
              source: 'ui',
              message: p.message.slice(0, 500),
              detail: typeof p.detail === 'string' ? p.detail.slice(0, 4000) : undefined,
            })
            return null
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
    element.addEventListener('load', attach)
    if (element.contentDocument?.readyState === 'complete') attach()
    return () => {
      element.removeEventListener('load', attach)
      dispose.current?.()
      dispose.current = null
      emit.current = null
    }
  }, [html, sessionId, pluginId, name, extension, context])

  if (error) return <div className="preview-plugin-state">{error}</div>
  return (
    <iframe
      ref={frame}
      className="preview-plugin-frame"
      sandbox="allow-scripts"
      srcDoc={html}
      title={name}
      data-contribution={contributionId}
    />
  )
}

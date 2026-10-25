import { useEffect, useRef, useState } from 'react'
import { LoaderCircle } from 'lucide-react'
import { call } from '@/lib/api'
import { themeSnapshot } from '@/lib/themes'
import { attachFiexPanel } from '../../../sdk/ui/src/host'
import type { JsonValue } from '../../../sdk/ui/src/index'

/** Standard plugin preview: Core returns a PreviewView plus the projected
 * PreviewContext; the iframe only sees the opaque session and resource
 * metadata, never a host path. */
export function PluginPreviewFrame({
  pluginId,
  contributionId,
  path,
  name,
  extension,
}: {
  pluginId: string
  contributionId: string
  path: string
  name: string
  extension: string
}) {
  const frame = useRef<HTMLIFrameElement>(null)
  const dispose = useRef<(() => void) | null>(null)
  const emit = useRef<((name: string, payload: JsonValue) => void) | null>(null)
  const [session, setSession] = useState<{
    sessionId: string
    context: JsonValue
    html: string
  } | null>(null)
  const [text, setText] = useState('')
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    let alive = true
    setSession(null)
    setText('')
    setMessage('')
    setError('')
    void call<{
      sessionId: string
      context: JsonValue
      view: { kind: string; html?: string; text?: string; message?: string }
    }>('preview_open', { path, pluginId, contributionId })
      .then((value) => {
        if (!alive) return
        if (value.view.kind === 'html' && value.view.html) {
          setSession({ sessionId: value.sessionId, context: value.context, html: value.view.html })
        } else if (value.view.kind === 'text') {
          setText(value.view.text || '')
        } else {
          setMessage(value.view.message || '此文件不支持插件预览')
        }
      })
      .catch((cause) => {
        if (alive) setError(String(cause))
      })
    return () => {
      alive = false
    }
  }, [path, pluginId, contributionId])

  useEffect(() => {
    const element = frame.current
    if (!session || !element) return
    let attached = false
    const attach = () => {
      if (attached) return
      attached = true
      dispose.current = attachFiexPanel({
        pluginId,
        sessionId: session.sessionId,
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
              ...(session.context as Record<string, JsonValue>),
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
              previewSessionId: session.sessionId,
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
            await call('clipboard.writeText', params as Record<string, unknown>)
            return null
          }
          if (method === 'navigation.open') {
            await call('navigation.open', params as Record<string, unknown>)
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
  }, [session, name, extension, pluginId])

  if (error) return <div className="preview-plugin-state">{error}</div>
  if (text) return <pre className="preview-text">{text}</pre>
  if (message) return <div className="preview-plugin-state">{message}</div>
  if (!session)
    return (
      <div className="preview-plugin-state">
        <LoaderCircle className="spin" size={16} /> 正在加载插件…
      </div>
    )
  return (
    <iframe
      ref={frame}
      className="preview-plugin-frame"
      sandbox="allow-scripts"
      srcDoc={session.html}
      title={name}
    />
  )
}

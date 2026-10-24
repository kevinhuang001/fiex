import { useEffect, useRef, useState } from 'react'
import { LoaderCircle } from 'lucide-react'
import { call } from '@/lib/api'
import { themeSnapshot } from '@/lib/themes'
import { attachFiexPanel } from '../../../sdk/ui/src/host'
import type { JsonValue } from '../../../sdk/ui/src/index'

const dirname = (path: string) => {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return index > 0 ? path.slice(0, index) : path
}

/** A plugin previewer rendered *inside* the preview sheet, with the same
 * sandbox and bridge as a panel. The file is passed as context; the plugin's
 * backend reads it through its own permissions. */
export function PluginPreviewFrame({
  pluginId,
  panelId,
  preview,
}: {
  pluginId: string
  panelId: string
  preview: { path: string; name: string; extension: string }
}) {
  const frame = useRef<HTMLIFrameElement>(null)
  const dispose = useRef<(() => void) | null>(null)
  const emit = useRef<((name: string, payload: JsonValue) => void) | null>(null)
  const [html, setHtml] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    let alive = true
    setHtml('')
    setError('')
    void call<string>('plugin_panel_html', { pluginId, panelId })
      .then((value) => {
        if (alive) setHtml(value)
      })
      .catch((cause) => {
        if (alive) setError(String(cause))
      })
    return () => {
      alive = false
    }
  }, [pluginId, panelId])

  useEffect(() => {
    const element = frame.current
    if (!html || !element) return
    const sessionId = crypto.randomUUID()
    const directory = dirname(preview.path)
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
        onError: (message) => setError(message),
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
          if (method === 'context.get')
            return {
              directoryPath: directory,
              selectionPaths: [preview.path],
              panelId,
              panelTitle: preview.name,
              source: 'preview',
              preview,
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
              directory,
              selectedPaths: [preview.path],
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
    // A srcdoc frame may already have loaded before this effect runs.
    if (element.contentDocument?.readyState === 'complete') attach()
    return () => {
      element.removeEventListener('load', attach)
      dispose.current?.()
      dispose.current = null
      emit.current = null
    }
  }, [html, pluginId, panelId, preview.path])

  if (error) return <div className="preview-plugin-state">{error}</div>
  if (!html)
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
      srcDoc={html}
      title={preview.name}
    />
  )
}

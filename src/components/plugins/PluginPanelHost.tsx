import { useEffect, useRef, useState } from 'react'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import { call } from '@/lib/api'
import { attachFiexPanel } from '../../../sdk/src/host'
import type { JsonValue } from '../../../sdk/src/index'

export interface PluginPanelRequest {
  pluginId: string
  panelId: string
  title: string
  selection: string[]
  directory: string
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
  const [size, setSize] = useState({ width: 620, height: 500 })
  const frame = useRef<HTMLIFrameElement>(null)
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
      setSize({ width: 620, height: 500 })
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
  const close = () => {
    dispose.current?.()
    dispose.current = null
    setRequest(null)
    setHtml('')
  }
  const attach = () => {
    if (!frame.current || !request) return
    dispose.current?.()
    const current = request
    try {
      dispose.current = attachFiexPanel({
        pluginId: current.pluginId,
        sessionId: crypto.randomUUID(),
        origin: 'null',
        frame: frame.current,
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
        ]),
        dispatch: async (method, params): Promise<JsonValue> => {
          if (method === 'context.get')
            return {
              directory: current.directory,
              selection: current.selection,
            }
          if (method === 'panel.close') {
            close()
            return null
          }
          if (method === 'panel.resize') {
            const p = (params || {}) as { width?: number; height?: number }
            if (Number.isFinite(p.width) && Number.isFinite(p.height)) {
              setSize({
                width: Math.min(1000, Math.max(360, Number(p.width))),
                height: Math.min(850, Math.max(240, Number(p.height))),
              })
            }
            return null
          }
          if (method === 'theme.get') return document.documentElement.dataset.theme || 'system'
          if (method === 'plugin.invoke') {
            const p = params as { method?: unknown; params?: unknown } | null
            if (!p || typeof p.method !== 'string' || !p.method || p.method.startsWith('host.'))
              throw new Error('Invalid plugin method')
            return await call<JsonValue>('plugin_invoke', {
              pluginId: current.pluginId,
              method: p.method,
              params: p.params || {},
            })
          }
          if (method.startsWith('dialog.')) {
            const granted = await call<boolean>('plugin_permission_allowed', {
              pluginId: current.pluginId,
              permission: 'ui.dialog',
            })
            if (!granted) throw new Error('插件文件选择权限未获授权，请在设置中重新授予')
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
  return (
    <>
      <Modal
        open={Boolean(request)}
        onClose={close}
        title={request?.title || '插件'}
        wide
        className="plugin-iframe-modal"
        contentStyle={{ width: size.width, maxWidth: '95vw' }}
      >
        {error ? (
          <p role="alert" className="connection-error">
            {error}
          </p>
        ) : !html ? (
          <p className="sidebar-hint">正在打开插件界面…</p>
        ) : (
          <iframe
            ref={frame}
            title={request?.title || '插件界面'}
            sandbox="allow-scripts"
            referrerPolicy="no-referrer"
            style={{ height: size.height, width: '100%', border: 0, borderRadius: 8 }}
            srcDoc={PANEL_CSP + html}
            onLoad={attach}
          />
        )}
      </Modal>
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

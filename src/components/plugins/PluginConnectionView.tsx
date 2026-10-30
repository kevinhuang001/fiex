import { t } from '@/i18n'
import { getLocale, subscribeLocale } from '@/i18n'
import { useEffect, useRef, useState } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { LoaderCircle } from 'lucide-react'
import { call, native } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { themeSnapshot } from '@/lib/themes'
import { attachFiexPanel } from '../../../sdk/ui/src/host'
import type { JsonValue } from '../../../sdk/ui/src/index'

type Destination =
  | { kind: 'vfs'; path: string }
  | {
      kind: 'pluginView'
      pluginId: string
      connectionId: string
      assetPath: string
      state: JsonValue
      tabTitle?: string
    }

/** PluginView replaces the entire right content area, excluding TabHost. */
export function PluginConnectionView({
  connectionId,
  onNavigate,
  onTitle,
}: {
  connectionId: string
  onNavigate: (path: string) => void
  onTitle?: (title: string) => void
}) {
  const frame = useRef<HTMLIFrameElement>(null)
  const navigate = useRef(onNavigate)
  navigate.current = onNavigate
  const [view, setView] = useState<Extract<Destination, { kind: 'pluginView' }> | null>(null)
  const [error, setError] = useState('')
  const emit = useRef<((name: string, payload: JsonValue) => void) | null>(null)

  useEffect(() => {
    let active = true
    setView(null)
    setError('')
    void call<Destination>('connection_open', { id: connectionId })
      .then((result) => {
        if (!active) return
        if (result.kind === 'vfs') {
          navigate.current(result.path)
        } else {
          setView(result)
          if (result.tabTitle) onTitle?.(result.tabTitle)
        }
      })
      .catch((cause) => {
        if (active) {
          setError(String(cause))
          reportError(cause, 'plugins:connection-view')
        }
      })
    return () => {
      active = false
    }
  }, [connectionId])

  useEffect(() => {
    const element = frame.current
    if (!view || !element) return
    let dispose: (() => void) | undefined
    let attached = false
    const attach = () => {
      if (attached) return
      attached = true
      dispose = attachFiexPanel({
        getLocale,
        subscribeLocale,
        pluginId: view.pluginId,
        sessionId: crypto.randomUUID(),
        origin: 'null',
        frame: element,
        allowedMethods: new Set([
          'context.get',
          'theme.get',
          'plugin.invoke',
          'plugin.error',
          'navigation.open',
        ]),
        onReady: () => emit.current?.('theme.changed', themeSnapshot()),
        onError: (message) => {
          setError(message)
          reportError(message, 'plugins:connection-view')
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
              pluginId: view.pluginId,
            }).catch(() => ({}))
            return {
              locale: getLocale(),
              source: 'connection',
              connectionId,
              state: view.state,
              settings,
            }
          }
          if (method === 'theme.get') return themeSnapshot()
          if (method === 'plugin.invoke') {
            const input = params as { method?: unknown; params?: unknown } | null
            if (
              !input ||
              typeof input.method !== 'string' ||
              !input.method ||
              /^(host|core|connection|vfs|fs)\./.test(input.method)
            ) {
              throw new Error('Invalid plugin method')
            }
            return call<JsonValue>('plugin_invoke', {
              pluginId: view.pluginId,
              method: input.method,
              params: input.params ?? null,
            })
          }
          if (method === 'navigation.open') {
            const input = params as { path?: unknown } | null
            if (!input || typeof input.path !== 'string')
              throw new Error('Invalid navigation target')
            navigate.current(input.path)
            return null
          }
          if (method === 'plugin.error') {
            const input = params as { message?: unknown; detail?: unknown } | null
            if (!input || typeof input.message !== 'string' || !input.message.trim())
              throw new Error('Invalid error message')
            await call('ui_error_report', {
              pluginId: view.pluginId,
              source: 'connection-ui',
              message: input.message.slice(0, 500),
              detail: typeof input.detail === 'string' ? input.detail.slice(0, 4000) : undefined,
            })
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
      dispose?.()
      emit.current = null
    }
  }, [view, connectionId])

  if (error)
    return (
      <div className="plugin-connection-message" role="status">
        {t('无法打开连接')}
      </div>
    )
  if (!view)
    return (
      <div className="plugin-connection-message" role="status">
        <LoaderCircle className="spin" size={18} /> {t('正在打开连接…')}
      </div>
    )
  return (
    <iframe
      key={view.assetPath}
      ref={frame}
      title={t('连接界面')}
      sandbox="allow-scripts"
      referrerPolicy="no-referrer"
      className="plugin-connection-frame"
      src={native ? convertFileSrc(view.assetPath) : view.assetPath}
    />
  )
}

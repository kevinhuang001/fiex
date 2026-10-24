import { convertFileSrc } from '@tauri-apps/api/core'
import { useEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { attachFiexPanel } from '../../../../sdk/ui/src/host'
import type { JsonValue } from '../../../../sdk/ui/src/index'
import { themeSnapshot } from '@/lib/themes'
import { call, native } from '@/lib/api'
import { openFilePicker } from '@/lib/filePicker'
import {
  beginActivity,
  cancelActivity,
  finishActivity,
  listActivity,
  updateActivity,
} from '@/hooks/useActivity'
import type { PluginPanelRequest } from './usePanelSession'
import { operationPaths } from './operationPaths'

export const MAX_PANEL_WIDTH = 720
export const MAX_PANEL_HEIGHT = 680

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
  void call('plugin_error_report', { pluginId, source, message: message.slice(0, 500) }).catch(
    () => {},
  )
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
    const req = request
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
        if (live) {
          setError(String(cause))
          reportPluginError(req.pluginId, 'panel', String(cause))
        }
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
    const chosenPaths = new Set<string>()
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
                ...(current.preview ? { preview: current.preview } : {}),
              }
            if (method === 'panel.close') {
              onClose()
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
                onSettleReveal()
              }
              return null
            }
            if (method === 'plugin.error') {
              const p = (params || {}) as { message?: unknown; detail?: unknown }
              if (typeof p.message !== 'string' || !p.message.trim())
                throw new Error('Invalid error message')
              await call('plugin_error_report', {
                pluginId: current.pluginId,
                source: 'ui',
                message: p.message.slice(0, 500),
                detail: typeof p.detail === 'string' ? p.detail.slice(0, 4000) : undefined,
              })
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
                selectedPaths: operationPaths(baseParams, [
                  ...current.selection,
                  ...chosenPaths,
                  ...(current.selection.length || chosenPaths.size ? [] : [current.directory]),
                ]),
              })
            }
            if (method === 'plugin.task') {
              const p = params as { taskId?: unknown } | null
              if (!p || typeof p.taskId !== 'string' || !p.taskId)
                throw new Error('Invalid plugin task id')
              return await call<JsonValue | null>('plugin_task', { taskId: p.taskId })
            }
            if (method.startsWith('dialog.')) {
              // The chooser is rendered by the host and only returns the path the
              // user explicitly picks; it never grants directory browsing.
              const p = (params || {}) as {
                directory?: string
                suggestedName?: string
                extensions?: string[]
              }
              // The chooser is the app's own file browser: the plugin only
              // receives the path the user explicitly picks.
              const picked = await openFilePicker({
                mode: method.slice('dialog.'.length) as
                  'openFile' | 'openFiles' | 'openDirectory' | 'saveFile',
                suggestedName: p.suggestedName,
                directory: p.directory || current.directory,
                extensions: p.extensions,
              })
              if (Array.isArray(picked)) picked.forEach((path) => chosenPaths.add(path))
              else if (typeof picked === 'string') chosenPaths.add(picked)
              if (method === 'dialog.openFiles')
                return Array.isArray(picked) ? picked : picked ? [picked] : null
              return typeof picked === 'string' ? picked : null
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
  return { frameRef, html, sourceUrl, ready, error, size, attach }
}

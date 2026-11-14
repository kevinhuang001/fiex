import { getLocale } from '@/i18n'
import {
  beginActivity,
  cancelActivity,
  finishActivity,
  listActivity,
  updateActivity,
} from '@/hooks/useActivity'
import { call } from '@/lib/api'
import { openFilePicker } from '@/lib/filePicker'
import { parseJsonValue } from '@/lib/ipcValidation'
import { themeSnapshot } from '@/lib/themes'
import type { ResourceRef, JsonValue } from '../../../../sdk/ui/src/index'
import type { PluginPanelRequest } from './usePanelSession'
import { PanelResources } from './resources'

export const MAX_PANEL_WIDTH = 720
export const MAX_PANEL_HEIGHT = 680

const METHODS_ALLOWED_WHEN_HIDDEN = new Set([
  'theme.get',
  'context.get',
  'tasks.update',
  'tasks.finish',
  // Cached panels can finish a measurement after hiding; resizing is harmless
  // and avoids a noisy bridge error during close/reopen transitions.
  'panel.resize',
])

export function canDispatchPanelMethod(method: string, active: boolean): boolean {
  return active || METHODS_ALLOWED_WHEN_HIDDEN.has(method)
}

export function initialPanelSize(
  requested: { width: number; height: number } | undefined,
  viewport: { width: number; height: number },
): { width: number; height: number } {
  const size = requested || { width: 560, height: 560 }
  return {
    width: Math.min(MAX_PANEL_WIDTH, size.width, viewport.width - 40),
    height: Math.min(MAX_PANEL_HEIGHT, size.height, viewport.height - 92),
  }
}

export function resizedPanelSize(
  requested: { width?: number; height?: number },
  viewport: { width: number; height: number },
): { width: number; height: number } | null {
  if (!Number.isFinite(requested.width) || !Number.isFinite(requested.height)) return null
  return {
    width: Math.min(MAX_PANEL_WIDTH, viewport.width - 40, Math.max(320, Number(requested.width))),
    height: Math.min(
      MAX_PANEL_HEIGHT,
      viewport.height - 92,
      Math.max(220, Number(requested.height)),
    ),
  }
}

interface PanelDispatcherOptions {
  current: PluginPanelRequest
  getActive: () => boolean
  resources: PanelResources
  taskIds: Set<string>
  onClose: () => void
  onSettleReveal: () => void
  setSize: (size: { width: number; height: number }) => void
}

export function createPanelDispatcher({
  current,
  getActive,
  resources,
  taskIds,
  onClose,
  onSettleReveal,
  setSize,
}: PanelDispatcherOptions) {
  return async (method: string, params: unknown): Promise<JsonValue> => {
    if (!canDispatchPanelMethod(method, getActive())) throw new Error('Panel is hidden')
    if (method === 'context.get') {
      const targets = await resources.describe(current.selection)
      const directory =
        current.allowCreate || current.selection.length === 0
          ? (await resources.describe([current.directory]))[0]
          : undefined
      const settings = await call('plugin_settings', { pluginId: current.pluginId }).catch(
        () => ({}),
      )
      return {
        locale: getLocale(),
        targets,
        ...(directory ? { directory } : {}),
        panelId: current.panelId,
        panelTitle: current.title,
        settings,
        uiTimeoutMs: current.timeoutMs ?? 15000,
        ...(current.source ? { source: current.source } : {}),
        ...(current.preview
          ? { preview: { name: current.preview.name, extension: current.preview.extension } }
          : {}),
      } as unknown as JsonValue
    }
    if (method === 'panel.close') {
      onClose()
      return null
    }
    if (method === 'panel.resize') {
      const size = resizedPanelSize((params || {}) as { width?: number; height?: number }, {
        width: window.innerWidth,
        height: window.innerHeight,
      })
      if (size) {
        setSize(size)
        onSettleReveal()
      }
      return null
    }
    if (method === 'plugin.error') {
      const p = (params || {}) as { message?: unknown; detail?: unknown }
      if (typeof p.message !== 'string' || !p.message.trim())
        throw new Error('Invalid error message')
      await call('ui_error_report', {
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
      if (!p || typeof p.path !== 'string' || !p.path) throw new Error('Invalid navigation path')
      window.dispatchEvent(
        new CustomEvent('fiex:navigate', { detail: { path: p.path, pane: 'left' } }),
      )
      return null
    }
    if (method === 'tasks.spinner') {
      const p = (params || {}) as { title?: unknown; detail?: unknown }
      if (typeof p.title !== 'string' || !p.title.trim()) throw new Error('Invalid task title')
      const id = beginActivity(
        p.title.trim(),
        typeof p.detail === 'string' ? p.detail : undefined,
        undefined,
        'plugin',
        current.pluginId,
      )
      taskIds.add(id)
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
      if (typeof p.title !== 'string' || !p.title.trim()) throw new Error('Invalid task title')
      const id = beginActivity(
        p.title.trim(),
        typeof p.detail === 'string' ? p.detail : undefined,
        undefined,
        'plugin',
        current.pluginId,
      )
      taskIds.add(id)
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
      if (typeof p.taskId !== 'string' || !taskIds.has(p.taskId)) throw new Error('Unknown task')
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
      if (typeof p.taskId !== 'string' || !taskIds.has(p.taskId)) throw new Error('Unknown task')
      finishActivity(
        p.taskId,
        typeof p.error === 'string' ? p.error : undefined,
        typeof p.detail === 'string' ? p.detail : undefined,
      )
      taskIds.delete(p.taskId)
      return null
    }
    if (method === 'tasks.cancel') {
      const p = (params || {}) as { taskId?: unknown }
      if (typeof p.taskId !== 'string' || !taskIds.has(p.taskId)) throw new Error('Unknown task')
      cancelActivity(p.taskId)
      taskIds.delete(p.taskId)
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
      const bindings = resources.bindings(baseParams)
      return await call('plugin_invoke', {
        pluginId: current.pluginId,
        method: p.method,
        params: parseJsonValue(baseParams),
        directory: current.directory,
        selectedPaths: bindings.map((binding) => binding.path),
        resourceBindings: bindings,
      })
    }
    if (method === 'plugin.task') {
      const p = params as { taskId?: unknown } | null
      if (!p || typeof p.taskId !== 'string' || !p.taskId) throw new Error('Invalid plugin task id')
      return await call('plugin_task', { pluginId: current.pluginId, taskId: p.taskId })
    }
    if (method.startsWith('dialog.')) {
      const p = (params || {}) as {
        directory?: ResourceRef
        suggestedName?: string
        extensions?: string[]
      }
      const picked = await openFilePicker({
        mode: method.slice('dialog.'.length) as
          'openFile' | 'openFiles' | 'openDirectory' | 'saveFile',
        suggestedName: p.suggestedName,
        directory: resources.path(p.directory) || current.directory,
        extensions: p.extensions,
      })
      const described = await resources.describe(
        Array.isArray(picked) ? picked : picked ? [picked] : [],
      )
      return (method === 'dialog.openFiles'
        ? described
        : described[0] || null) as unknown as JsonValue
    }
    throw new Error('Unsupported panel operation')
  }
}

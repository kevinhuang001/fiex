import { t } from '@/i18n'
import { listen } from '@tauri-apps/api/event'
import { useEffect } from 'react'
import { call, native } from '@/lib/api'
import { beginActivity, finishActivity, updateActivity } from './useActivity'

interface PluginTaskEvent {
  taskId: string
  pluginId: string
  label: string
  status: 'running' | 'cancelling' | 'completed' | 'failed' | 'cancelled'
  done: number
  total?: number | null
  result?: unknown
  error?: string | null
  internal?: boolean
}

function detailOf(result: unknown): string | undefined {
  if (typeof result === 'string') return result
  if (result === null || result === undefined) return undefined
  return JSON.stringify(result)
}

/**
 * Mirrors Core-owned plugin background tasks into the desktop status centre.
 * Cancellation travels back through Core, which notifies the resident plugin;
 * the plugin remains responsible for cooperative cancellation.
 */
export function usePluginTasks() {
  useEffect(() => {
    if (!native) return
    const activities = new Map<string, string>()
    let disposed = false
    const unlisten = listen<PluginTaskEvent>('plugin-task', ({ payload }) => {
      if (!payload || typeof payload.taskId !== 'string') return
      if (payload.internal) return
      if (payload.status === 'running') {
        let activity = activities.get(payload.taskId)
        if (!activity) {
          activity = beginActivity(
            payload.label || t('插件后台任务'),
            undefined,
            () => {
              void call('plugin_cancel_task', {
                pluginId: payload.pluginId,
                taskId: payload.taskId,
              }).catch(() => {})
            },
            'plugin',
            payload.pluginId,
          )
          activities.set(payload.taskId, activity)
        }
        updateActivity(activity, {
          done: payload.done,
          total: payload.total ?? undefined,
          progress: payload.total && payload.total > 0 ? payload.done / payload.total : undefined,
          indeterminate: !payload.total,
        })
        return
      }
      const activity = activities.get(payload.taskId)
      if (!activity) return
      if (payload.status === 'cancelling') {
        updateActivity(activity, { detail: t('正在取消…'), indeterminate: true })
        return
      }
      activities.delete(payload.taskId)
      const error = payload.status === 'cancelled' ? t('已取消') : (payload.error ?? undefined)
      finishActivity(activity, error, detailOf(payload.result))
    })
    return () => {
      disposed = true
      if (disposed) void unlisten.then((stop) => stop())
    }
  }, [])
}

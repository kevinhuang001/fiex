import { t } from '@/i18n'
/** Duration formatting and ETA estimation for the activity centre. */
export function formatRemaining(seconds: number) {
  const value = Math.max(1, Math.ceil(seconds))
  if (value < 60) return t('{0} 秒', value)
  const minutes = Math.floor(value / 60)
  if (minutes < 60) return t('{0} 分 {1} 秒', minutes, value % 60)
  return t('{0} 小时 {1} 分', Math.floor(minutes / 60), minutes % 60)
}

/** Estimate the remaining time from the completed item count and elapsed time. */
export function activityEta(
  item: {
    status: string
    total?: number
    done?: number
    startedAt: number
  },
  now = Date.now(),
) {
  if (item.status !== 'running' || !item.total || !item.done || item.done >= item.total) return null
  const elapsed = now - item.startedAt
  if (elapsed < 50) return null
  const remaining = (elapsed / item.done) * (item.total - item.done)
  if (!Number.isFinite(remaining) || remaining <= 0) return null
  return formatRemaining(remaining / 1000)
}

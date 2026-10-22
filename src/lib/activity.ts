/** Duration formatting and ETA estimation for the activity centre. */
export function formatRemaining(seconds: number) {
  const value = Math.max(1, Math.ceil(seconds))
  if (value < 60) return `${value} 秒`
  const minutes = Math.floor(value / 60)
  if (minutes < 60) return `${minutes} 分 ${value % 60} 秒`
  return `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分`
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

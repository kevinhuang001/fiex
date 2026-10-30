/** One global UI notification queue, separate from persistent diagnostics.
 * Normal cancellation and internal fallbacks should not call this API. */
export interface AppFailure {
  id: number
  source: string
  message: string
  at: number
}
let queue: AppFailure[] = []
let nextId = 0
const listeners = new Set<() => void>()
const publish = () => {
  for (const listener of listeners) listener()
}
export const subscribeErrors = (listener: () => void) => {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}
export const activeError = () => queue[0] ?? null
export function reportError(cause: unknown, source = 'core') {
  const message = String(cause ?? '')
    .trim()
    .slice(0, 2000)
  if (!message) return
  const now = Date.now()
  if (
    queue.some((item) => item.source === source && item.message === message && now - item.at < 3000)
  )
    return
  queue = [...queue.slice(-49), { id: ++nextId, source, message, at: now }]
  publish()
}
export function dismissError(id: number) {
  queue = queue.filter((item) => item.id !== id)
  publish()
}

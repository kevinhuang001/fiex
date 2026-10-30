import { redactDiagnostic } from './redactDiagnostic'
/** One global UI notification queue, separate from persistent diagnostics.
 * Normal cancellation and internal fallbacks should not call this API. */
export interface AppFailure {
  id: number
  source: string
  message: string
  at: number
  diagnosticId?: string
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
export function reportError(cause: unknown, source = 'core', diagnosticId?: string) {
  const message = redactDiagnostic(String(cause ?? ''), 2000)
  if (!message) return
  const now = Date.now()
  if (queue.some((item) => item.source === source && item.message === message)) return
  if (queue.length >= 50) return
  queue = [...queue, { id: ++nextId, source, message, at: now, diagnosticId }]
  publish()
}
export function dismissError(id: number) {
  queue = queue.filter((item) => item.id !== id)
  publish()
}

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
/** Identify permission failures returned by the OS without rewriting the
 * original message. Provider capability failures remain normal unsupported
 * errors; local and USB filesystem permissions are reported after execution. */
export function isFilesystemPermissionError(cause: unknown) {
  const message = String(cause ?? '').toLowerCase()
  return (
    /permission denied|permissiondenied:|operation not permitted|read-only file system|read only file system|access is denied|access denied/.test(
      message,
    ) || /\bos error (1|5|13|30|1314)\b/.test(message)
  )
}
export function reportFilesystemPermissionError(cause: unknown, context: string) {
  if (!isFilesystemPermissionError(cause)) return false
  reportError(`${context}\n${String(cause ?? '')}`, 'filesystem')
  return true
}
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

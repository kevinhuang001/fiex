import { call, native } from '@/lib/api'
import { useSyncExternalStore } from 'react'

export interface DisplayError {
  id: string
  message: string
  code: string
  source: string
  operation: string
  at: number
}
let queue: DisplayError[] = []
const listeners = new Set<() => void>()
const notify = () => {
  listeners.forEach((listener) => listener())
}
export function reportError(
  message: string,
  code = 'ui.failure',
  operation = 'ui',
  source = 'desktop',
) {
  if (!message.trim()) return
  const at = Date.now()
  if (queue.some((item) => item.code === code && item.message === message && at - item.at < 15_000))
    return
  queue = [
    ...queue,
    {
      id: `${at}:${queue.length}`,
      at,
      message: message.slice(0, 2000),
      code,
      source,
      operation,
    },
  ].slice(-32)
  notify()
  if (native) {
    void call('core_error_report', { code, operation, source, message }).catch(() => {})
  }
}
export function dismissError() {
  if (!queue.length) return
  queue = queue.slice(1)
  notify()
}
function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}
function snapshot() {
  return queue[0] ?? null
}
export function useCurrentError() {
  return useSyncExternalStore(subscribe, snapshot, () => null)
}
export async function receiveCoreErrors() {
  if (!native) return
  const errors = await call<
    Array<{
      id: number
      message: string
      code: string
      source: string
      operation: string
      at: number
    }>
  >('error_poll')
  for (const error of errors) {
    if (!queue.some((item) => item.id === `core:${error.id}`)) {
      queue = [...queue, { ...error, id: `core:${error.id}` }].slice(-32)
    }
  }
  if (errors.length) notify()
}

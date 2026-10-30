import { useSyncExternalStore } from 'react'
let state: string | null = null
const listeners = new Set<() => void>()
function publish(value: string | null) {
  state = value
  for (const listener of listeners) listener()
}
export const showLogs = (source = 'core') => publish(source)
export const closeLogs = () => publish(null)
export function useLogSource() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    () => state,
    () => null,
  )
}

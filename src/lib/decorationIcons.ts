import { useCallback, useSyncExternalStore } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { native } from '@/lib/api'
import { cachedPresentation, presentationKey } from './entryPresentation'
import type { Entry } from './types'

const listeners = new Map<string, Set<() => void>>()
const pending = new Set<string>()
function isApplication(entry: Entry): boolean {
  return ['app', 'exe', 'desktop', 'appimage'].includes(entry.extension)
}
export function publishPresentationIcons(entries: Entry[], presentationPending?: boolean) {
  for (const entry of entries) {
    const key = presentationKey(entry)
    if (presentationPending === true && isApplication(entry)) pending.add(key)
    else if (presentationPending === false) pending.delete(key)
    for (const callback of listeners.get(key) ?? []) callback()
  }
}
export function useDecorationIcon(entry: Entry): string | undefined {
  const key = presentationKey(entry)
  const subscribe = useCallback(
    (listener: () => void) => {
      const callbacks = listeners.get(key) ?? new Set<() => void>()
      callbacks.add(listener)
      listeners.set(key, callbacks)
      return () => {
        callbacks.delete(listener)
        if (!callbacks.size) listeners.delete(key)
      }
    },
    [key],
  )
  const snapshot = useCallback(() => {
    const path = cachedPresentation(entry)?.iconPath
    return path ? (native ? convertFileSrc(path) : path) : undefined
  }, [key])
  return useSyncExternalStore(subscribe, snapshot, () => undefined)
}

export function usePresentationPending(entry: Entry): boolean {
  const key = presentationKey(entry)
  const subscribe = useCallback(
    (listener: () => void) => {
      const callbacks = listeners.get(key) ?? new Set<() => void>()
      callbacks.add(listener)
      listeners.set(key, callbacks)
      return () => {
        callbacks.delete(listener)
        if (!callbacks.size) listeners.delete(key)
      }
    },
    [key],
  )
  const snapshot = useCallback(() => pending.has(key), [key])
  return useSyncExternalStore(subscribe, snapshot, () => false)
}

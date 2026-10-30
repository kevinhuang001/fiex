import { useCallback, useSyncExternalStore } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { native } from '@/lib/api'
import { cachedPresentation, presentationKey } from './entryPresentation'
import type { Entry } from './types'

const listeners = new Map<string, Set<() => void>>()
export function publishPresentationIcons(entries: Entry[]) {
  for (const entry of entries)
    for (const callback of listeners.get(presentationKey(entry)) ?? []) callback()
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

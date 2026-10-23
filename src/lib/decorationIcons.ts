import { useSyncExternalStore } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { native } from '@/lib/api'
import type { EntryDecoration } from './types'

let icons = new Map<string, string>()
let version = 0
const listeners = new Set<() => void>()

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Keep a path-indexed icon map in sync with the current decoration response. */
export function syncDecorationIcons(decorations: Record<string, EntryDecoration>) {
  const next = new Map<string, string>()
  for (const [path, decoration] of Object.entries(decorations)) {
    if (decoration.iconPath)
      next.set(path, native ? convertFileSrc(decoration.iconPath) : decoration.iconPath)
  }
  icons = next
  version += 1
  for (const listener of listeners) listener()
}

export function useDecorationIcon(path: string): string | undefined {
  useSyncExternalStore(
    subscribe,
    () => version,
    () => 0,
  )
  return icons.get(path)
}

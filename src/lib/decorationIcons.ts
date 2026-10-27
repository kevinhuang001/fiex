import { useSyncExternalStore } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { native } from '@/lib/api'
import type { EntryDecoration } from './types'

let icons = new Map<string, string>()
const owners = new Map<string, Map<string, string>>()
const listeners = new Map<string, Set<() => void>>()

function subscribe(path: string, listener: () => void) {
  const callbacks = listeners.get(path) || new Set<() => void>()
  callbacks.add(listener)
  listeners.set(path, callbacks)
  return () => {
    callbacks.delete(listener)
    if (!callbacks.size) listeners.delete(path)
  }
}

function publish() {
  const next = new Map<string, string>()
  for (const values of owners.values()) for (const [path, url] of values) next.set(path, url)
  const previous = icons
  icons = next
  for (const path of new Set([...previous.keys(), ...next.keys()])) {
    if (previous.get(path) !== next.get(path))
      for (const callback of listeners.get(path) || []) callback()
  }
}

/** Keep a path-indexed icon map in sync with the current decoration response. */
export function syncDecorationIcons(decorations: Record<string, EntryDecoration>, owner: string) {
  const own = new Map<string, string>()
  for (const [path, decoration] of Object.entries(decorations)) {
    if (decoration.iconPath)
      own.set(path, native ? convertFileSrc(decoration.iconPath) : decoration.iconPath)
  }
  owners.set(owner, own)
  publish()
}

export function clearDecorationIcons(owner: string) {
  owners.delete(owner)
  publish()
}

export function useDecorationIcon(path: string): string | undefined {
  return useSyncExternalStore(
    (listener) => subscribe(path, listener),
    () => icons.get(path),
    () => undefined,
  )
}

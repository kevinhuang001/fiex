import { call } from './api'
import type { Listing } from './types'

// Multiple panes, the column browser and the picker can ask for the same
// directory concurrently. Share only in-flight work: a subsequent navigation
// or explicit refresh must always observe new filesystem and sync state.
const pending = new Map<string, Promise<Listing>>()

export function listDirectory(
  path: string,
  hidden = false,
  ignores: string[] = [],
): Promise<Listing> {
  const key = JSON.stringify([path, hidden, ignores])
  const existing = pending.get(key)
  if (existing) return existing
  const request = call<Listing>('list_directory', { path, hidden, ignores })
  pending.set(key, request)
  void request.then(
    () => {
      if (pending.get(key) === request) pending.delete(key)
    },
    () => {
      if (pending.get(key) === request) pending.delete(key)
    },
  )
  return request
}

import { call, native } from './api'
import { Channel } from '@tauri-apps/api/core'
import type { Listing, EntryDecoration } from './types'

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

export interface DirectoryFrame {
  listing: Listing
  decorations: EntryDecoration[]
  done: boolean
  warning?: string | null
}

/** One navigation operation. Core owns the cache-first, live-revalidation and
 * presentation merge. React only receives normal directory snapshots. */
export function openDirectory(
  path: string,
  hidden: boolean,
  ignores: string[],
  onFrame: (frame: DirectoryFrame) => void,
): Promise<void> {
  if (!native) {
    return listDirectory(path, hidden, ignores).then((listing) => {
      onFrame({ listing, decorations: [], done: true })
    })
  }
  const channel = new Channel<DirectoryFrame>()
  channel.onmessage = onFrame
  return call<void>('directory_open', { path, hidden, ignores, onSnapshot: channel })
}

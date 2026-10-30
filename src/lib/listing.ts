import { cachePresentations } from './entryPresentation'
import { publishPresentationIcons } from './decorationIcons'
import { call, native } from './api'
import { Channel } from '@tauri-apps/api/core'
import type { EntryDecoration } from './types'
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

export interface DirectoryViewUpdate {
  listing: Listing
  decorations: EntryDecoration[]
  complete: boolean
}

/** One logical navigation call. Core owns the cache, live revalidation and
 * presentation query; callers receive ordinary snapshots, not cache sources. */
export function openDirectoryStream(
  path: string,
  hidden: boolean,
  ignores: string[],
  onSnapshot: (value: DirectoryViewUpdate) => void,
): Promise<void> {
  if (!native) {
    return listDirectory(path, hidden, ignores).then((listing) =>
      onSnapshot({ listing, decorations: [], complete: true }),
    )
  }
  const channel = new Channel<DirectoryViewUpdate>()
  channel.onmessage = onSnapshot
  return call<void>('directory_open', { path, hidden, ignores, onSnapshot: channel })
}

export function openPresentedDirectory(
  path: string,
  hidden: boolean,
  ignores: string[],
  onListing: (listing: Listing) => void,
): Promise<void> {
  return openDirectoryStream(path, hidden, ignores, (snapshot) => {
    cachePresentations(snapshot.listing.entries, snapshot.decorations)
    publishPresentationIcons(snapshot.listing.entries)
    onListing(snapshot.listing)
  })
}

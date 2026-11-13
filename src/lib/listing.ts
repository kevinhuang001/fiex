import { cachePresentationPatch, cachePresentations } from './entryPresentation'
import { publishPresentationIcons } from './decorationIcons'
import { call, native } from './api'
import { parseDirectoryViewUpdate } from './ipcValidation'
import { reportError } from './errorCenter'
import { Channel } from '@tauri-apps/api/core'
import type { Entry, EntryDecoration, Listing } from './types'

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
  const request = call('list_directory', { path, hidden, ignores })
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
  listing?: Listing
  decorations: EntryDecoration[]
  complete: boolean
  presentationPending: boolean
  revision: number
}

/** Ask every visible pane showing one of these directories to reconcile its
 * current snapshot without restarting its initial-loading lifecycle. */
export function requestDirectoryRefresh(directories: string[]) {
  const unique = [...new Set(directories.filter(Boolean))]
  if (!unique.length || typeof window === 'undefined') return
  window.dispatchEvent(
    new CustomEvent('fiex:directory-mutated', { detail: { directories: unique } }),
  )
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
      onSnapshot({
        listing,
        decorations: [],
        complete: true,
        presentationPending: false,
        revision: 0,
      }),
    )
  }
  const channel = new Channel<unknown>()
  channel.onmessage = (raw) => {
    try {
      onSnapshot(parseDirectoryViewUpdate(raw))
    } catch (cause) {
      reportError(cause, 'explorer:directory-ipc')
    }
  }
  return call('directory_open', { path, hidden, ignores, onSnapshot: channel })
}

export function openPresentedDirectory(
  path: string,
  hidden: boolean,
  ignores: string[],
  onListing: (listing: Listing) => void,
): Promise<void> {
  let entries: Entry[] = []
  return openDirectoryStream(path, hidden, ignores, (snapshot) => {
    if (snapshot.listing) {
      entries = snapshot.listing.entries
      cachePresentations(entries, snapshot.decorations)
      publishPresentationIcons(entries, snapshot.presentationPending)
      onListing(snapshot.listing)
    } else {
      cachePresentationPatch(entries, snapshot.decorations)
      publishPresentationIcons(entries, snapshot.presentationPending)
    }
  })
}

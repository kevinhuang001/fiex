import { cachePresentationPatch, cachePresentations } from './entryPresentation'
import { publishPresentationIcons } from './decorationIcons'
import { call, native } from './api'
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
  listing?: Listing
  decorations: EntryDecoration[]
  complete: boolean
  presentationPending: boolean
}

/** Ask every visible pane showing one of these directories to apply its
 * cache-first snapshot and complete the usual live-list reconciliation. */
export function requestDirectoryRefresh(directories: string[]) {
  const unique = [...new Set(directories.filter(Boolean))]
  if (!unique.length || typeof window === 'undefined') return
  window.dispatchEvent(
    new CustomEvent('fiex:directory-mutated', { detail: { directories: unique } }),
  )
}

export function parentDirectoryOf(path: string): string {
  const scheme = path.indexOf('://')
  if (scheme >= 0) {
    const rootEnd = scheme + 3
    const trimmed = path.replace(/\/+$/, '')
    const separator = trimmed.lastIndexOf('/')
    return separator < rootEnd ? path.slice(0, rootEnd) : trimmed.slice(0, separator)
  }
  const trimmed = path.replace(/[\\/]+$/, '')
  const separator = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'))
  if (separator < 0) return ''
  if (separator === 0) return trimmed.slice(0, 1)
  if (separator === 2 && /^[A-Za-z]:/.test(trimmed)) return trimmed.slice(0, 3)
  return trimmed.slice(0, separator)
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
      onSnapshot({ listing, decorations: [], complete: true, presentationPending: false }),
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

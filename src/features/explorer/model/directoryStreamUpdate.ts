import { reconcileDirectorySnapshot } from './directorySnapshot'
import type { PaintedDirectorySnapshot } from './paintedListingCache'
import type { Entry, EntryDecoration, Listing, Sort } from '@/lib/types'

export interface DirectoryStreamUpdate {
  listing?: Listing
  decorations: EntryDecoration[]
  complete: boolean
  presentationPending: boolean
  revision: number
}

export interface PendingDirectorySelection {
  directory: string
  target: string
  generation: number
}

export type DirectorySelectionResolution =
  | { kind: 'none' }
  | { kind: 'select'; target: string; generation: number }
  | { kind: 'missing'; generation: number }

export type DirectoryStreamDecision =
  | { kind: 'ignore' }
  | { kind: 'navigate'; path: string }
  | {
      kind: 'patch'
      snapshot: PaintedDirectorySnapshot
      patchDecorations: EntryDecoration[]
      presentationPending: boolean
      shouldApplyDecorations: boolean
      redirectHome: boolean
    }
  | {
      kind: 'listing'
      snapshot: PaintedDirectorySnapshot
      entries: Entry[]
      visibleChanged: boolean
      decorations: EntryDecoration[]
      selection: DirectorySelectionResolution
      redirectHome: boolean
    }

/** Chooses how one directory-stream update affects the current Explorer view. */
export function decideDirectoryStreamUpdate(input: {
  path: string
  update: DirectoryStreamUpdate
  previous: PaintedDirectorySnapshot | null
  sort: Sort
  descending: boolean
  historyLength: number
  pendingSelection: PendingDirectorySelection | null
}): DirectoryStreamDecision {
  const { path, update, previous, sort, descending, historyLength, pendingSelection } = input
  if (previous && update.revision < previous.revision) return { kind: 'ignore' }

  if (!update.listing) {
    if (!previous) return { kind: 'ignore' }
    const decorations = new Map(previous.decorations.map((value) => [value.path, value]))
    for (const value of update.decorations) decorations.set(value.path, value)
    const mergedDecorations = [...decorations.values()]
    return {
      kind: 'patch',
      snapshot: { ...previous, decorations: mergedDecorations, revision: update.revision },
      patchDecorations: update.decorations,
      presentationPending: update.presentationPending,
      shouldApplyDecorations: update.decorations.length > 0,
      redirectHome: shouldRedirectToHome(
        path,
        update.complete,
        previous.listing.entries.length,
        historyLength,
      ),
    }
  }

  if (update.listing.path !== path) return { kind: 'navigate', path: update.listing.path }

  const selection: DirectorySelectionResolution =
    pendingSelection?.directory !== path
      ? { kind: 'none' }
      : update.listing.entries.some((entry) => entry.path === pendingSelection.target)
        ? {
            kind: 'select',
            target: pendingSelection.target,
            generation: pendingSelection.generation,
          }
        : update.complete
          ? { kind: 'missing', generation: pendingSelection.generation }
          : { kind: 'none' }
  const reconciled = reconcileDirectorySnapshot(
    previous,
    update.listing,
    update.decorations,
    update.revision,
    sort,
    descending,
  )

  return {
    kind: 'listing',
    snapshot: reconciled.snapshot,
    entries: reconciled.entries,
    visibleChanged: reconciled.visibleChanged,
    decorations: update.decorations,
    selection,
    redirectHome: shouldRedirectToHome(
      path,
      update.complete,
      update.listing.entries.length,
      historyLength,
    ),
  }
}

function shouldRedirectToHome(
  path: string,
  complete: boolean,
  entryCount: number,
  historyLength: number,
): boolean {
  return path === 'recent://' && complete && entryCount === 0 && historyLength <= 1
}

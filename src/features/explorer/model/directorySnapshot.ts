import { sameVisibleListing } from './listingDiff'
import { sameListingMetadata } from './paintedListingCache'
import type { PaintedDirectorySnapshot } from './paintedListingCache'
import type { Entry, EntryDecoration, Listing, Sort } from '@/lib/types'

export interface DirectorySnapshotReconciliation {
  snapshot: PaintedDirectorySnapshot
  entries: Entry[]
  visibleChanged: boolean
}

/** Reuses rendered entry identity while refreshing listing metadata and decoration state. */
export function reconcileDirectorySnapshot(
  previous: PaintedDirectorySnapshot | null,
  listing: Listing,
  decorations: EntryDecoration[],
  revision: number,
  sort: Sort,
  descending: boolean,
): DirectorySnapshotReconciliation {
  const visibleUnchanged =
    previous !== null &&
    sameVisibleListing(previous.listing.entries, listing.entries, sort, descending)
  const entries = previous && visibleUnchanged ? previous.listing.entries : listing.entries
  const nextListing = { ...listing, entries }
  const stableListing =
    previous && visibleUnchanged && sameListingMetadata(previous.listing, nextListing)
      ? previous.listing
      : nextListing

  return {
    snapshot: { listing: stableListing, decorations, revision },
    entries,
    visibleChanged: !visibleUnchanged,
  }
}

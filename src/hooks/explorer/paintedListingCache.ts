import { sameDecoration } from '@/lib/entryPresentation'
import { sameStringSet } from '@/lib/listingDiff'
import type { EntryDecoration, Listing } from '@/lib/types'

export interface PaintedDirectorySnapshot {
  listing: Listing
  decorations: EntryDecoration[]
  revision: number
}

/** Bounded cache for directories already painted by the explorer. */
export class PaintedListingCache {
  readonly #snapshots = new Map<string, PaintedDirectorySnapshot>()

  constructor(private readonly capacity: number) {
    if (!Number.isSafeInteger(capacity) || capacity < 1) {
      throw new RangeError('Painted listing cache capacity must be a positive integer')
    }
  }

  get(key: string): PaintedDirectorySnapshot | null {
    return this.#snapshots.get(key) ?? null
  }

  touch(key: string): PaintedDirectorySnapshot | null {
    const snapshot = this.get(key)
    if (!snapshot) return null
    this.#snapshots.delete(key)
    this.#snapshots.set(key, snapshot)
    return snapshot
  }

  remember(key: string, snapshot: PaintedDirectorySnapshot): void {
    this.#snapshots.delete(key)
    this.#snapshots.set(key, snapshot)
    while (this.#snapshots.size > this.capacity) {
      const oldest = this.#snapshots.keys().next().value
      if (oldest === undefined) return
      this.#snapshots.delete(oldest)
    }
  }
}

export function sameListingMetadata(previous: Listing, next: Listing): boolean {
  return (
    previous.path === next.path &&
    sameStringSet(previous.allowedOps, next.allowedOps) &&
    previous.virtualVolume === next.virtualVolume &&
    previous.defaultView === next.defaultView &&
    previous.warnings.length === next.warnings.length &&
    previous.warnings.every((warning, index) => warning === next.warnings[index])
  )
}

export function sameDirectoryDecorations(
  previous: EntryDecoration[],
  next: EntryDecoration[],
): boolean {
  if (previous.length !== next.length) return false
  const nextByPath = new Map(next.map((value) => [value.path, value]))
  return previous.every((value) => {
    const other = nextByPath.get(value.path)
    return other !== undefined && sameDecoration(value, other)
  })
}

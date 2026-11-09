import { sameListing } from '@/lib/listingDiff'
import type { Batch, Entry } from '@/lib/types'

/** Reconcile one streamed search batch without publishing unchanged entries. */
export function mergeSearchBatch(previous: Entry[], batch: Batch): Entry[] {
  if (batch.replace) return sameListing(previous, batch.entries) ? previous : batch.entries
  if (!batch.entries.length) return previous

  const merged = new Map(previous.map((entry) => [entry.path, entry]))
  let changed = false
  for (const entry of batch.entries) {
    if (JSON.stringify(merged.get(entry.path)) !== JSON.stringify(entry)) {
      merged.set(entry.path, entry)
      changed = true
    }
  }
  return changed ? [...merged.values()] : previous
}

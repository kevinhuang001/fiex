import type { Entry, EntryDecoration } from '@/lib/types'

/** Builtins consume listing metadata only: no reads, downloads or new grants. */
export interface DecorationProvider {
  id: string
  priority: number
  resolve: (entry: Entry) => EntryDecoration | undefined
}

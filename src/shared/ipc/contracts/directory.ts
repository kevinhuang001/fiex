import type { EntryDecoration, Listing } from '@/lib/types'

export interface DirectoryViewUpdate {
  listing?: Listing
  decorations: EntryDecoration[]
  complete: boolean
  presentationPending: boolean
  revision: number
}

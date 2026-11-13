import type { DirectoryViewUpdate } from '../listing'
import { parseEntryDecorations } from './decorations'
import { objectResult } from './json'
import { parseListing } from './entries'

export function parseDirectoryViewUpdate(value: unknown): DirectoryViewUpdate {
  const update = objectResult(value)
  if (
    !Array.isArray(update.decorations) ||
    typeof update.complete !== 'boolean' ||
    typeof update.presentationPending !== 'boolean' ||
    typeof update.revision !== 'number'
  )
    throw new TypeError('Invalid directory update')
  return {
    ...(update.listing === undefined || update.listing === null
      ? {}
      : { listing: parseListing(update.listing) }),
    decorations: parseEntryDecorations(update.decorations),
    complete: update.complete,
    presentationPending: update.presentationPending,
    revision: update.revision,
  }
}

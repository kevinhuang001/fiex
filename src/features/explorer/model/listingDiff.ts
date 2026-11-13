import { sortEntries } from '@/lib/files'
import type { Entry, Sort } from '@/lib/types'

/** Compare rendered directory data by stable path identity, independent of
 * provider response order. Callers can keep the current array when this holds. */
export function sameListing(previous: Entry[], next: Entry[]): boolean {
  if (previous.length !== next.length) return false
  const nextByPath = new Map(next.map((entry) => [entry.path, entry]))
  if (nextByPath.size !== next.length) return false
  return previous.every((entry) => {
    const value = nextByPath.get(entry.path)
    return (
      value !== undefined &&
      entry.name === value.name &&
      entry.isDir === value.isDir &&
      entry.isSymlink === value.isSymlink &&
      entry.size === value.size &&
      entry.modified === value.modified &&
      entry.created === value.created &&
      entry.extension === value.extension &&
      entry.mime === value.mime &&
      entry.linkTarget === value.linkTarget &&
      entry.hasCache === value.hasCache &&
      entry.pinned === value.pinned &&
      entry.uploaded === value.uploaded &&
      entry.sync === value.sync &&
      sameStringSet(entry.allowedOps, value.allowedOps) &&
      entry.tags.length === value.tags.length &&
      entry.tags.every(
        (tag, index) =>
          tag.name === value.tags[index].name &&
          tag.color === value.tags[index].color &&
          tag.hex === value.tags[index].hex,
      )
    )
  })
}

/** Operation arrays are capability sets; their serialization order is not
 * visible state and should not invalidate a directory snapshot. */
export function sameStringSet(previous: string[] | undefined, next: string[] | undefined): boolean {
  return (
    previous === next ||
    (previous !== undefined &&
      next !== undefined &&
      previous.length === next.length &&
      previous.every((value) => next.includes(value)))
  )
}

/** Compare the entries in the order the active view actually displays them.
 * Provider response order is irrelevant when the user's sort produces the same
 * visible order, including stable tie ordering. */
export function sameVisibleListing(
  previous: Entry[],
  next: Entry[],
  sort: Sort,
  descending: boolean,
): boolean {
  if (!sameListing(previous, next)) return false
  const previousOrder = sortEntries(previous, sort, descending)
  const nextOrder = sortEntries(next, sort, descending)
  return previousOrder.every((entry, index) => entry.path === nextOrder[index]?.path)
}

import { getLocale } from '@/i18n'
import type { Entry, Group, Sort } from '../types'
import { groupKey, kind } from './metadata'

const collators = new Map<string, Intl.Collator>()
const collator = {
  compare: (a: string, b: string) => {
    const locale = getLocale()
    if (!collators.has(locale))
      collators.set(locale, new Intl.Collator(locale, { numeric: true, sensitivity: 'base' }))
    return collators.get(locale)!.compare(a, b)
  },
}
export function sortEntries(entries: Entry[], sort: Sort, desc: boolean): Entry[] {
  return [...entries].sort(
    (a, b) =>
      Number(b.isDir) - Number(a.isDir) ||
      (desc ? -1 : 1) *
        (sort === 'name'
          ? collator.compare(a.name, b.name)
          : sort === 'kind'
            ? collator.compare(kind(a), kind(b)) || collator.compare(a.name, b.name)
            : a[sort] - b[sort] || collator.compare(a.name, b.name)),
  )
}
/** Sort for a clicked list header: a different column starts ascending, the
 *  active column flips direction. Folder-first ordering stays in sortEntries. */
export function nextSort(
  sort: Sort,
  descending: boolean,
  key: Sort,
): { sort: Sort; descending: boolean } {
  return { sort: key, descending: sort === key ? !descending : false }
}
export function groups(entries: Entry[], by: Group): [string, Entry[]][] {
  const map = new Map<string, Entry[]>()
  for (const e of entries) {
    const key = groupKey(e, by)
    if (!map.has(key)) map.set(key, [])
    map.get(key)!.push(e)
  }
  return [...map]
}
export function isImage(e: Entry) {
  return (
    e.mime.startsWith('image/') &&
    !['icns', 'ico', 'heic', 'heif', 'tif', 'tiff', 'psd', 'raw'].includes(e.extension)
  )
}

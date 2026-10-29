/** Presentation is part of an entry snapshot, shared by every FS/VFS view.
 * Returning to a directory can render labels, badges, styles and icons in the
 * same React render as its cached entries. An async refresh never clears it. */
import { getLocale } from '@/i18n'
import type { Entry, EntryDecoration } from './types'

const values = new Map<string, EntryDecoration>()
export function presentationKey(entry: Entry): string {
  return JSON.stringify([getLocale(), entry])
}
export function cachedPresentation(entry: Entry): EntryDecoration | undefined {
  return values.get(presentationKey(entry))
}
export function cachePresentations(entries: Entry[], decorations: EntryDecoration[]) {
  const byPath = new Map(decorations.map((decoration) => [decoration.path, decoration]))
  for (const entry of entries) {
    const key = presentationKey(entry)
    values.delete(key)
    values.set(key, byPath.get(entry.path) ?? { path: entry.path, badges: [] })
  }
  while (values.size > 10_000) values.delete(values.keys().next().value!)
}
export function cachedPresentations(entries: Entry[]): Record<string, EntryDecoration> {
  return Object.fromEntries(
    entries.flatMap((entry) => {
      const value = cachedPresentation(entry)
      return value ? [[entry.path, value]] : []
    }),
  )
}

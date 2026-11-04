/** Presentation is part of an entry snapshot, shared by every FS/VFS view.
 * Returning to a directory can render labels, badges, styles and icons in the
 * same React render as its cached entries. An async refresh never clears it. */
import { getLocale } from '@/i18n'
import type { Entry, EntryDecoration } from './types'

const values = new Map<string, EntryDecoration>()
export function presentationKey(entry: Entry): string {
  return JSON.stringify([
    getLocale(),
    entry.path,
    entry.name,
    entry.isDir,
    entry.isSymlink,
    entry.size,
    entry.modified,
    entry.extension,
    entry.mime,
    entry.allowedOps,
    entry.linkTarget,
    entry.tags,
    entry.hasCache,
    entry.pinned,
    entry.uploaded,
    entry.sync,
  ])
}
export function cachedPresentation(entry: Entry): EntryDecoration | undefined {
  const key = presentationKey(entry)
  const value = values.get(key)
  if (value) {
    values.delete(key)
    values.set(key, value)
  }
  return value
}
export function cachePresentations(entries: Entry[], decorations: EntryDecoration[]) {
  const byPath = new Map(decorations.map((decoration) => [decoration.path, decoration]))
  for (const entry of entries) {
    const key = presentationKey(entry)
    values.delete(key)
    values.set(key, byPath.get(entry.path) ?? { path: entry.path })
  }
  while (values.size > 10_000) values.delete(values.keys().next().value!)
}
export function cachePresentationPatch(entries: Entry[], decorations: EntryDecoration[]) {
  if (!decorations.length) return
  const byPath = new Map(entries.map((entry) => [entry.path, entry]))
  for (const decoration of decorations) {
    const entry = byPath.get(decoration.path)
    if (!entry) continue
    const key = presentationKey(entry)
    values.delete(key)
    values.set(key, decoration)
  }
  while (values.size > 10_000) values.delete(values.keys().next().value!)
}
export function sameDecoration(left: EntryDecoration, right: EntryDecoration): boolean {
  return (
    left.path === right.path &&
    left.label === right.label &&
    left.description === right.description &&
    left.icon === right.icon &&
    left.iconPath === right.iconPath &&
    JSON.stringify(left.nativeBadges || []) === JSON.stringify(right.nativeBadges || []) &&
    JSON.stringify(left.fields || {}) === JSON.stringify(right.fields || {}) &&
    JSON.stringify(left.providers || []) === JSON.stringify(right.providers || [])
  )
}
export function cachedPresentations(entries: Entry[]): Record<string, EntryDecoration> {
  return Object.fromEntries(
    entries.flatMap((entry) => {
      const value = cachedPresentation(entry)
      return value ? [[entry.path, value]] : []
    }),
  )
}

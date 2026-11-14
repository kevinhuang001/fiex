import { getLocale, t, type Locale } from '@/i18n'
import { kind } from '@/lib/files'
import type { FilePickerMode } from '@/lib/filePicker'
import type { Entry } from '@/lib/types'
import type { GroupKey, PickerSort } from './types'

type PickerGroupLabel = { label: string; rank: number }
export type PickerGroup = { label: string | null; entries: Entry[] }

function pickerGroup(entry: Entry, group: GroupKey, now: number): PickerGroupLabel {
  if (group === 'kind')
    return entry.isDir ? { label: t('文件夹'), rank: 0 } : { label: kind(entry), rank: 1 }
  if (group === 'size') {
    if (entry.isDir) return { label: t('文件夹'), rank: 0 }
    if (entry.size === 0) return { label: t('空'), rank: 1 }
    if (entry.size < 1024 * 1024) return { label: t('小于 1 MB'), rank: 2 }
    if (entry.size < 100 * 1024 * 1024) return { label: t('小于 100 MB'), rank: 3 }
    return { label: t('大于 100 MB'), rank: 4 }
  }
  if (group === 'modified') {
    const age = now - entry.modified * 1000
    if (age < 24 * 60 * 60 * 1000) return { label: t('今天'), rank: 0 }
    if (age < 48 * 60 * 60 * 1000) return { label: t('昨天'), rank: 1 }
    if (age < 7 * 24 * 60 * 60 * 1000) return { label: t('过去 7 天'), rank: 2 }
    if (age < 30 * 24 * 60 * 60 * 1000) return { label: t('过去 30 天'), rank: 3 }
    return { label: t('更早'), rank: 4 }
  }
  return { label: '', rank: 0 }
}

export function visiblePickerEntries(
  entries: Entry[],
  filter: string,
  mode: FilePickerMode,
  extensions: string[] | undefined,
  sort: PickerSort,
  group: GroupKey,
  now = Date.now(),
  locale: Locale = getLocale(),
): Entry[] {
  const query = filter.trim().toLowerCase()
  const allowedExtensions = mode === 'saveFile' ? [] : extensions || []
  const filtered = entries.filter((entry) => {
    if (query && !entry.name.toLowerCase().includes(query)) return false
    if (!allowedExtensions.length || entry.isDir) return true
    return allowedExtensions.some((extension) => {
      const suffix = extension.startsWith('.') ? extension : `.${extension}`
      return entry.name.toLowerCase().endsWith(suffix.toLowerCase())
    })
  })
  const direction = sort.direction === 'asc' ? 1 : -1
  return [...filtered].sort((left, right) => {
    if (group !== 'none') {
      const leftGroup = pickerGroup(left, group, now)
      const rightGroup = pickerGroup(right, group, now)
      if (leftGroup.rank !== rightGroup.rank) return leftGroup.rank - rightGroup.rank
      if (leftGroup.label !== rightGroup.label)
        return leftGroup.label.localeCompare(rightGroup.label, locale)
    }
    if (left.isDir !== right.isDir) return left.isDir ? -1 : 1
    const result =
      sort.key === 'name'
        ? left.name.localeCompare(right.name, locale)
        : sort.key === 'modified'
          ? left.modified - right.modified
          : sort.key === 'size'
            ? left.size - right.size
            : kind(left).localeCompare(kind(right), locale)
    return result * direction
  })
}

export function groupPickerEntries(
  entries: Entry[],
  group: GroupKey,
  now = Date.now(),
): PickerGroup[] {
  if (group === 'none') return [{ label: null, entries }]
  const sections: PickerGroup[] = []
  for (const entry of entries) {
    const label = pickerGroup(entry, group, now).label
    const last = sections[sections.length - 1]
    if (last && last.label === label) last.entries.push(entry)
    else sections.push({ label, entries: [entry] })
  }
  return sections
}

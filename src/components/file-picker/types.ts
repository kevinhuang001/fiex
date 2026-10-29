import { stateSnapshot, saveState } from '@/lib/stateStore'

export type SortKey = 'name' | 'modified' | 'size' | 'kind'
export type GroupKey = 'none' | 'kind' | 'modified' | 'size'

export function storedSort(): { key: SortKey; direction: 'asc' | 'desc' } {
  const value = stateSnapshot<{ key: SortKey; direction: 'asc' | 'desc' }>(
    'picker.sort', 'fiex.picker.sort',
  )
  if (value && ['name', 'modified', 'size', 'kind'].includes(value.key))
    return { key: value.key, direction: value.direction === 'desc' ? 'desc' : 'asc' }
  return { key: 'name', direction: 'asc' }
}
export function storedGroup(): GroupKey {
  const value = stateSnapshot<GroupKey>('picker.group', 'fiex.picker.group')
  return value && ['none', 'kind', 'modified', 'size'].includes(value) ? value : 'none'
}
export function persistPickerSort(
  sort: { key: SortKey; direction: 'asc' | 'desc' },
  group: GroupKey,
) {
  saveState('picker.sort', sort)
  saveState('picker.group', group)
}
export function storedDirectory(): string {
  return stateSnapshot<string>('picker.lastDirectory', 'fiex.picker.directory') || ''
}
export function persistPickerDirectory(path: string) {
  if (path) saveState('picker.lastDirectory', path)
}

export const dirname = (path: string) => {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return index > 0 ? path.slice(0, index) : path
}

export const join = (directory: string, name: string) =>
  `${directory.replace(/[/\\]$/, '')}/${name}`.replace(/\/+/g, '/')

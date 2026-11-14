export type SortKey = 'name' | 'modified' | 'size' | 'kind'
export type PickerSort = { key: SortKey; direction: 'asc' | 'desc' }
export type GroupKey = 'none' | 'kind' | 'modified' | 'size'

export const dirname = (path: string) => {
  if (/^(?:\\\\|\/\/)[^\\/]+[\\/][^\\/]+[\\/]*$/.test(path)) return path
  const driveRoot = /^[a-z]:[\\/]/i.exec(path)?.[0]
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  if (index < 0) return path
  if (driveRoot && index <= 2) return driveRoot
  if (index === 0) return path[0] || path
  return path.slice(0, index)
}

export const join = (directory: string, name: string) =>
  `${directory.replace(/[/\\]$/, '')}/${name}`.replace(/\/+/g, '/')

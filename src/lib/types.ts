export interface Entry {
  path: string
  name: string
  isDir: boolean
  isSymlink: boolean
  size: number
  modified: number
  created: number
  extension: string
  mime: string
  readonly: boolean
}
export interface Listing {
  path: string
  entries: Entry[]
  warnings: string[]
}
export interface Location {
  name: string
  path: string
  kind: string
  available: number
  total: number
}
export interface Terminal {
  id: string
  name: string
}
export interface Batch {
  entries: Entry[]
  done: boolean
  elapsedMs: number
  truncated: boolean
  skipped: number
}
export interface OperationResult {
  paths: string[]
  errors: string[]
}
export type View = 'list' | 'gallery' | 'columns' | 'icons'
export type Group = 'none' | 'kind' | 'modified' | 'extension' | 'size'
export type Sort = 'name' | 'modified' | 'size' | 'kind'
export interface Preferences {
  view: View
  group: Group
  sort: Sort
  descending: boolean
  hidden: boolean
  respectIgnore: boolean
  terminal: string
  theme: 'system' | 'light' | 'dark'
  iconSize: number
  favorites: Location[]
}

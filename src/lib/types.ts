import type { ShortcutMap, ShortcutStyle } from './shortcuts'

export interface Tag {
  name: string
  color: number
}
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
  linkTarget: string | null
  tags: Tag[]
  sync: 'synced' | 'cloud' | null
}
export interface Listing {
  path: string
  entries: Entry[]
  warnings: string[]
}
export type SidebarSection = 'places' | 'favorites' | 'tags' | 'devices'

export interface Location {
  name: string
  path: string
  kind: string
  available: number
  total: number
  removable: boolean
  filesystem: string
  device: string
  mounted: boolean
  usbfs: boolean
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
  skipped: number
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
  terminal: string
  theme: 'system' | 'light' | 'dark'
  fontSize: number
  displayIgnore: string[]
  compressIgnore: string[]
  copyIgnore: string[]
  sidebarOrder: SidebarSection[]
  placeOrder: string[]
  hiddenPlaces: string[]
  favorites: Location[]
  shortcutStyle: ShortcutStyle
  shortcuts: ShortcutMap
}

export interface GitStatus {
  root: string
  branch: string
  entries: Record<string, string>
}

export interface RemoteConnection {
  id: string
  name: string
  kind: 'webdav' | 'smb'
  mountPath: string
  host: string
  url: string
  share: string
  base: string
}

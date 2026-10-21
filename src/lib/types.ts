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
  readOnly?: boolean
  virtualVolume?: boolean
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
export interface Ide {
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
  conflicts?: TransferConflict[]
}
export interface FileVersion {
  path: string
  size: number
  modified: number
  isDir: boolean
}
export interface TransferConflict {
  key: string
  source: FileVersion
  destination: FileVersion
}
export type ConflictAction = 'replace' | 'skip' | 'keepBoth'
export interface ConflictResolution {
  key: string
  action: ConflictAction
}
export interface GitRemoteStatus {
  remotes: { name: string; url: string }[]
  branch: string
  upstream: string
  ahead: number
  behind: number
}
export type View = 'list' | 'gallery' | 'columns' | 'icons'
export type Group = 'none' | 'kind' | 'modified' | 'extension' | 'size'
export type Sort = 'name' | 'modified' | 'size' | 'kind'
export type StartPage = 'recent' | 'home' | 'documents' | 'last'

export interface Preferences {
  customTags: Tag[]
  view: View
  group: Group
  sort: Sort
  descending: boolean
  hidden: boolean
  terminal: string
  ide: string
  theme: 'system' | 'light' | 'dark'
  fontSize: number
  startPage: StartPage
  displayIgnore: string[]
  compressIgnore: string[]
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

export interface GitRefs {
  head: string
  current: string
  branches: string[]
  tags: string[]
}

export interface GitCommit {
  refs: string
  id: string
  shortId: string
  subject: string
  author: string
  timestamp: number
}

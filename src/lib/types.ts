import type { ShortcutMap, ShortcutStyle } from './shortcuts'

export interface Tag {
  name: string
  /** Finder tag colour index (0–7). */
  color: number
  /** Optional exact colour chosen in Fiex for custom tags. */
  hex?: string
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
  sync: 'synced' | 'cloud' | 'pinned' | 'pending' | 'conflict' | null
}
export interface Listing {
  readOnly?: boolean
  virtualVolume?: boolean
  path: string
  entries: Entry[]
  warnings: string[]
}
export type SidebarSection = 'places' | 'favorites' | 'tags' | 'devices' | 'connections'

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
export interface DecorationBadge {
  text: string
  color: string
  tooltip?: string
}
export interface DecorationStyle {
  color?: string
  fontWeight?: string
  fontStyle?: string
  backgroundLight?: string
  backgroundDark?: string
}
export interface DecorationField {
  badges?: DecorationBadge[]
  style?: DecorationStyle
}
export type DecorationTarget = 'name' | 'modified' | 'size' | 'kind' | 'row'
export interface EntryDecoration {
  path: string
  badges: DecorationBadge[]
  label?: string
  description?: string
  style?: DecorationStyle
  fields?: Partial<Record<DecorationTarget, DecorationField>>
  icon?: string
  iconPath?: string
  providers?: string[]
}
export type View = 'list' | 'gallery' | 'columns' | 'icons'
export type Group = 'none' | 'kind' | 'modified' | 'extension' | 'size'
export type Sort = 'name' | 'modified' | 'size' | 'kind'
export type StartPage = 'recent' | 'home' | 'documents' | 'last'

export interface Preferences {
  tags: Tag[]
  view: View
  group: Group
  sort: Sort
  descending: boolean
  hidden: boolean
  terminal: string
  ide: string
  theme: 'system' | 'light' | 'dark'
  themeId?: string
  fontSize: number
  startPage: StartPage
  displayIgnore: string[]
  sidebarOrder: SidebarSection[]
  placeOrder: string[]
  hiddenPlaces: string[]
  hiddenSections: SidebarSection[]
  toolbarLeftOrder: string[]
  toolbarRightOrder: string[]
  restoreTabs: boolean
  favorites: Location[]
  shortcutStyle: ShortcutStyle
  shortcuts: ShortcutMap
  pinnedPluginTools: string[]
}

export interface OperationProgress {
  done: number
  total: number
  step?: string
}

export interface GitStatus {
  root: string
  branch: string
  entries: Record<string, string>
}

export interface RemoteConnection {
  id: string
  name: string
  kind: string
  pluginId: string
  mountPath: string
  saved?: boolean
  autostart?: boolean
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

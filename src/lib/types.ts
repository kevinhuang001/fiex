import type { ShortcutMap, ShortcutStyle } from './shortcuts'
import type { ListColumns } from './listColumns'

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
  /** Explicit storage operations; absence is unknown, an empty set grants none. */
  allowedOps?: string[]
  linkTarget: string | null
  tags: Tag[]
  /** This entry, or a directory descendant, has cached bytes. */
  hasCache?: boolean
  /** "Always keep locally" is enabled for a connection entry. */
  pinned?: boolean
  /** The current bytes of a connection entry reached the remote. */
  uploaded?: boolean
  sync: 'synced' | 'cloud' | 'pinned' | 'pending' | 'conflict' | null
}
export interface Listing {
  readOnly?: boolean
  /** The current directory accepts `mkdir` (provider capability ∩ node). */
  canCreate?: boolean
  virtualVolume?: boolean
  path: string
  entries: Entry[]
  warnings: string[]
  /** Preferred initial view when this folder has no remembered state. */
  defaultView?: View
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
export interface DecorationBadge {
  /** Only Core builtin decoration providers emit these presentation fields. */
  dot?: boolean
  status?: 'synced' | 'cloud' | 'pinned' | 'pending' | 'conflict' | 'uploaded'
  icon?: 'cloud-check' | 'cloud-download' | 'cloud-upload' | 'pin'

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
  nativeBadges?: DecorationBadge[]
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
type StartPage = 'recent' | 'home' | 'documents' | 'last'

export interface Preferences {
  locale: import('@/i18n').Locale
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
  backgroundImage: string
  backgroundOpacity: number
  fontSize: number
  terminalFont: string
  terminalFontSize: number
  terminalColor: string
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
  /** Background connection reconciliation cadence, in seconds. */
  syncInterval: number
  /** List-view column widths; the flexible name column takes the rest. */
  listColumns: ListColumns
}

export interface OperationProgress {
  done: number
  total: number
  step?: string
  /** `bytes` renders sizes instead of file counts. */
  unit?: 'bytes' | 'files'
}

export interface RemoteConnection {
  id: string
  name: string
  kind: string
  pluginId: string
  mountPath: string
  saved?: boolean
  autostart?: boolean
  /** Provider can enumerate and read this mount. */
  canRead?: boolean
  /** Provider can upload/create inside this mount. */
  canWrite?: boolean
}

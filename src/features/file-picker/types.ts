import type { Entry } from '@/lib/types'
import type { FilePickerMode } from '@/lib/filePicker'
import type { Listing, Location, RemoteConnection } from '@/lib/types'

export type SortKey = 'name' | 'modified' | 'size' | 'kind'
export type PickerSort = { key: SortKey; direction: 'asc' | 'desc' }
export type GroupKey = 'none' | 'kind' | 'modified' | 'size'

export type PickerDialogState =
  | { kind: 'folder'; value: string }
  | { kind: 'rename'; entry: Entry; value: string }
  | { kind: 'trash'; entry: Entry }

export interface PickerMenuState {
  x: number
  y: number
  entry?: Entry
  kind: 'row' | 'blank'
}

export type PickerStateWrite =
  | { key: 'sort'; value: PickerSort }
  | { key: 'group'; value: GroupKey }
  | { key: 'lastSaveDirectory' | 'lastOpenDirectory'; value: string }

export interface PickerStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

export interface PickerPreferenceServices {
  storage: PickerStorage
  writeState(write: PickerStateWrite): Promise<unknown>
}

export interface PickerSidebarData {
  locations: Location[]
  connections: RemoteConnection[]
}

export interface PickerDirectoryServices {
  preferences: PickerPreferenceServices
  readSettings(mode: FilePickerMode): Promise<{ sort: unknown; group: unknown; directory: unknown }>
  sidebarSnapshot(): PickerSidebarData | null
  loadSidebarData(): Promise<PickerSidebarData>
  openDirectory(
    path: string,
    hidden: boolean,
    ignores: string[],
    onListing: (listing: Listing) => void,
  ): Promise<void>
  reportError(cause: unknown, source: string): void
}

export interface PickerOperationServices {
  complete(value: string | string[] | null): void
  createFolder(directory: string, name: string): Promise<string>
  rename(path: string, name: string): Promise<unknown>
  recycle(path: string): Promise<unknown>
  reportError(cause: unknown, source: string): void
}

export interface PickerSessionEnvironment {
  favorites: Location[]
  directory: PickerDirectoryServices
  operations: PickerOperationServices
}

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

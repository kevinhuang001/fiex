import type { useMenuPosition } from '@/lib/menuPosition'
import type { Location, Preferences, RemoteConnection } from '@/lib/types'
import type { Dispatch, SetStateAction } from 'react'

export interface SidebarExplorer {
  path: string
  locations: Location[]
  connections: RemoteConnection[]
  tabs: string[]
  setError: (error: string) => void
  refreshConnections: () => Promise<void>
  reload: () => void
  navigate: (path: string) => void
  tagFilter: string | null
  setTagFilter: (tag: string | null) => void
}

export interface SidebarProps {
  x: SidebarExplorer
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
  onTrash: () => void
  onApplications: () => void
  onAddConnection: () => void
  onEditConnection: (connection: RemoteConnection) => void
  onRemoveConnection: (id: string) => void
  notify: (message: string) => void
  onEject: (path: string) => void
  onEjectDevice: (device: string) => void
  onMount: (device: string) => void
  trashPath: string | null
}

export type SidebarMenuState =
  | { kind: 'place'; x: number; y: number; key: string; name: string; trash: boolean }
  | { kind: 'favorite'; x: number; y: number; path: string }
  | {
      kind: 'device'
      x: number
      y: number
      name: string
      device: string
      path: string
      mounted: boolean
      usbfs: boolean
      canEject: boolean
    }

export type SetSidebarMenu = Dispatch<SetStateAction<SidebarMenuState | null>>

export type MenuPosition = ReturnType<typeof useMenuPosition>

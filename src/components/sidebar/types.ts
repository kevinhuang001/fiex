import type { Explorer } from '@/hooks/useExplorer'
import type { useMenuPosition } from '@/lib/menuPosition'
import type { Preferences, RemoteConnection } from '@/lib/types'
import type { Dispatch, SetStateAction } from 'react'

export interface SidebarProps {
  x: Explorer
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
  | { kind: 'trash'; x: number; y: number }
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

import type { TransferConflict } from '@/lib/types'
import type { Run } from '@/features/explorer/operations/types'

/** File-operation helpers shared by the controller hooks. */
export type { Run }
export type Notify = (message: string) => void
export type Transfer = (paths: string[], destination: string, moving: boolean) => void
export type ConflictState = {
  paths: string[]
  destination: string
  moving: boolean
  items: TransferConflict[]
} | null

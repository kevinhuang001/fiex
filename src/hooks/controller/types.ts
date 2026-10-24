import type { TransferConflict } from '@/lib/types'

/** File-operation helpers shared by the controller hooks. */
export type Run = <T>(label: string, fn: () => Promise<T>) => Promise<T | undefined>
export type Notify = (message: string) => void
export type Transfer = (paths: string[], destination: string, moving: boolean) => void
export type ConflictState = {
  paths: string[]
  destination: string
  moving: boolean
  items: TransferConflict[]
} | null

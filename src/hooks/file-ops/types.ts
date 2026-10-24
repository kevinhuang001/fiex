import type { DialogState } from '@/lib/dialogs'
import type { OperationResult, TransferConflict } from '@/lib/types'
import type { ActivityKind } from '@/hooks/useActivity'
import type { Explorer } from '@/hooks/useExplorer'
import type { Dispatch, SetStateAction } from 'react'

export type Clipboard = { paths: string[]; moving: boolean } | null

export type ConflictState = {
  paths: string[]
  destination: string
  moving: boolean
  items: TransferConflict[]
} | null

export type Run = <T>(
  label: string,
  fn: () => Promise<T>,
  detail?: string,
  cancellable?: boolean,
  activityKind?: ActivityKind,
  onActivity?: (id: string) => void,
) => Promise<T | undefined>

export type HandleResult = (result: OperationResult) => void

/** Everything the file-operation groups need: the pane, its feedback channels
 *  and the state shared between transfer, recycle, create and restore. */
export interface FileOpsContext {
  x: Explorer
  notify: (message: string) => void
  setDialog: Dispatch<SetStateAction<DialogState | null>>
  writable: () => boolean
  inTrash: boolean
  conflict: ConflictState
  setConflict: Dispatch<SetStateAction<ConflictState>>
  setClipboard: Dispatch<SetStateAction<Clipboard>>
  run: Run
  handleResult: HandleResult
}

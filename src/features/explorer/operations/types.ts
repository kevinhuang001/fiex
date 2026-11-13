import type { DialogState } from '@/lib/dialogs'
import type { Channel } from '@tauri-apps/api/core'
import type { Entry, OperationProgress, OperationResult, TransferConflict } from '@/lib/types'
import type { Dispatch, SetStateAction } from 'react'

export type ActivityKind = 'transfer' | 'delete' | 'sync' | 'plugin'

export interface ExplorerOperationState {
  path: string
  query: string
  entries: Entry[]
  selected: string[]
  directoryAllowedOps: string[]
  setError: (message: string) => void
  setQuery: (query: string) => void
  setSelected: (paths: string[]) => void
  beginRename: (target: string, createdEntry?: Entry) => void
}

export interface OperationActivity {
  progressChannel: (id: string) => Channel<OperationProgress>
  quietProgressChannel: () => Channel<OperationProgress>
  updateActivity: (id: string, patch: { detail?: string; indeterminate?: boolean }) => void
}

export type Clipboard = { paths: string[]; moving: boolean } | null

export type ConflictState = {
  paths: string[]
  destination: string
  moving: boolean
  items: TransferConflict[]
  /** The still-running activity row that the resolution will complete. */
  activityId?: string
} | null

export type Run = <T>(
  label: string,
  fn: () => Promise<T>,
  detail?: string,
  cancellable?: boolean,
  activityKind?: ActivityKind,
  onActivity?: (id: string) => void,
  /** Record the activity without opening the panel. */
  quiet?: boolean,
  /** When set, the caller keeps the activity running (for a pending dialog). */
  hold?: { value: boolean },
  /** Continue an activity that a conflict dialog left running. */
  resume?: string,
  /** Directly name directories changed by this operation; cancelled or failed
   * operations can return the same known scope to reconcile partial work. */
  affectedDirectories?: (result: T | undefined) => string[],
) => Promise<T | undefined>

export type HandleResult = (result: OperationResult) => void

/** Everything the file-operation groups need: the pane, its feedback channels
 *  and the state shared between transfer, recycle, create and restore. */
export interface FileOpsContext {
  x: ExplorerOperationState
  notify: (message: string) => void
  setDialog: Dispatch<SetStateAction<DialogState | null>>
  inTrash: boolean
  conflict: ConflictState
  setConflict: Dispatch<SetStateAction<ConflictState>>
  setClipboard: Dispatch<SetStateAction<Clipboard>>
  run: Run
  handleResult: HandleResult
  activity: OperationActivity
}

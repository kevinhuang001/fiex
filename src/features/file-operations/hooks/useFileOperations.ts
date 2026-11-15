import { useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import type { DialogState } from '@/lib/dialogs'
import { createActions } from '../create'
import { recycleActions } from '../recycle'
import { createResultHandler } from '../result'
import { restoreActions } from '../restore'
import { transferActions } from '../transfer'
import type {
  Clipboard,
  ConflictState,
  ExplorerOperationState,
  FileOperationRuntimeServices,
  FileOperationServices,
} from '../types'
import { useRunner } from './useRunner'

/** Composes operation models while receiving every app/OS effect from its caller. */
export function useFileOperations({
  x,
  notify,
  setDialog,
  trashPath,
  services,
  runtime,
}: {
  x: ExplorerOperationState
  notify: (message: string) => void
  setDialog: Dispatch<SetStateAction<DialogState | null>>
  trashPath: string | null
  services: FileOperationServices
  runtime: FileOperationRuntimeServices
}) {
  const inTrash = Boolean(trashPath && (x.path === trashPath || x.path.startsWith(trashPath + '/')))
  const [busy, setBusy] = useState('')
  const [conflict, setConflict] = useState<ConflictState>(null)
  const [clipboard, setClipboard] = useState<Clipboard>(null)
  const { run } = useRunner({ x, setBusy, runtime, services })
  const context = {
    x,
    notify,
    setDialog,
    inTrash,
    conflict,
    setConflict,
    setClipboard,
    run,
    services,
    handleResult: createResultHandler(notify),
    activity: runtime,
  }
  const { create, createFile } = createActions(context)
  const { recycle, deletePermanently, emptyTrash } = recycleActions(context)
  const { restoreFromTrash, undo } = restoreActions(context)
  const { transfer, resolveConflict, copy, duplicate } = transferActions(context)
  return {
    busy,
    conflict,
    setConflict,
    resolveConflict,
    clipboard,
    setClipboard,
    run,
    create,
    createFile,
    recycle,
    deletePermanently,
    restoreFromTrash,
    emptyTrash,
    inTrash,
    transfer,
    copy,
    duplicate,
    undo,
  }
}

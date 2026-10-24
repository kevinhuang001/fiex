import type { DialogState } from '@/lib/dialogs'
import type { Explorer } from '@/hooks/useExplorer'
import { useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { createActions } from './create'
import { recycleActions } from './recycle'
import { createResultHandler } from './result'
import { restoreActions } from './restore'
import { useRunner } from './run'
import { transferActions } from './transfer'
import type { Clipboard, ConflictState } from './types'

export function useFileOperations({
  x,
  writable,
  notify,
  setDialog,
  trashPath,
}: {
  x: Explorer
  writable: () => boolean
  notify: (message: string) => void
  setDialog: Dispatch<SetStateAction<DialogState | null>>
  trashPath: string | null
}) {
  // Inside the Trash, "move to Trash" would be a no-op, so the delete action
  // becomes permanent just like the system file manager.
  const inTrash = Boolean(trashPath && (x.path === trashPath || x.path.startsWith(trashPath + '/')))
  const [busy, setBusy] = useState('')
  const [conflict, setConflict] = useState<ConflictState>(null)
  const [clipboard, setClipboard] = useState<Clipboard>(null)
  const { run } = useRunner({ x, setBusy })
  const handleResult = createResultHandler(notify)
  const context = {
    x,
    notify,
    setDialog,
    writable,
    inTrash,
    conflict,
    setConflict,
    setClipboard,
    run,
    handleResult,
  }
  const { create } = createActions(context)
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

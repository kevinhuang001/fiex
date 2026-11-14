import type { DialogState } from '@/lib/dialogs'
import type { Explorer } from '@/features/explorer/hooks/useExplorer'
import { progressChannel, quietProgressChannel, updateActivity } from '@/hooks/useActivity'
import { useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { createActions } from '@/features/explorer/operations/create'
import { recycleActions } from '@/features/explorer/operations/recycle'
import { createResultHandler } from '@/features/explorer/operations/result'
import { restoreActions } from '@/features/explorer/operations/restore'
import type {
  Clipboard,
  ConflictState,
  FileOperationServices,
} from '@/features/explorer/operations/types'
import { transferActions } from '@/features/explorer/operations/transfer'
import { call } from '@/lib/api'
import { openFilePicker } from '@/lib/filePicker'
import { reportFilesystemPermissionError } from '@/lib/errorCenter'
import { useRunner } from './run'

export function useFileOperations({
  x,
  notify,
  setDialog,
  trashPath,
}: {
  x: Explorer
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
  const services: FileOperationServices = {
    fileInfo: (path) => call('file_info', { path }),
    createEntry: (path, name) => call('create_entry', { path, name }),
    createFileEntry: (path, name, template) => call('create_file_entry', { path, name, template }),
    recycle: (paths, onProgress) => call('recycle', { paths, onProgress }),
    deletePermanently: (paths) => call('delete_permanently', { paths }),
    emptyTrash: () => call('empty_trash'),
    trashRestoreNeedsFolder: (paths) => call('trash_restore_needs_folder', { paths }),
    restoreFromTrash: (paths, fallback) => call('restore_from_trash', { paths, fallback }),
    undo: () => call('undo'),
    pickDirectory: async () => {
      const picked = await openFilePicker({ mode: 'openDirectory' })
      return typeof picked === 'string' ? picked : null
    },
    reportPermissionError: reportFilesystemPermissionError,
  }
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
    handleResult,
    activity: { progressChannel, quietProgressChannel, updateActivity },
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

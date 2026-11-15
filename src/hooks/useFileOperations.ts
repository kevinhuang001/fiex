import type { DialogState } from '@/lib/dialogs'
import type { Explorer } from '@/features/explorer/hooks/useExplorer'
import {
  beginActivity,
  finishActivity,
  progressChannel,
  quietProgressChannel,
  updateActivity,
} from '@/hooks/useActivity'
import { useFileOperations as useFeatureFileOperations } from '@/features/file-operations/hooks/useFileOperations'
import type {
  FileOperationRuntimeServices,
  FileOperationServices,
} from '@/features/file-operations/types'
import { call } from '@/lib/api'
import { reportFilesystemPermissionError } from '@/lib/errorCenter'
import { openFilePicker } from '@/lib/filePicker'
import { requestDirectoryRefresh } from '@/lib/listing'
import type { Dispatch, SetStateAction } from 'react'

/** Binds Tauri, activity, refresh, and picker effects around the file feature. */
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
    fileParent: (path) => call('file_parent', { path }),
    transfer: (request) => call('transfer', request),
    pickDirectory: async () => {
      const picked = await openFilePicker({ mode: 'openDirectory' })
      return typeof picked === 'string' ? picked : null
    },
    reportPermissionError: reportFilesystemPermissionError,
  }
  const runtime: FileOperationRuntimeServices = {
    progressChannel,
    quietProgressChannel,
    updateActivity,
    beginOperation: async () => {
      await call('begin_operation').catch(() => {})
    },
    cancelOperation: async () => {
      await call('cancel_operation').catch(() => {})
    },
    beginActivity,
    finishActivity,
    requestDirectoryRefresh,
  }
  return useFeatureFileOperations({ x, notify, setDialog, trashPath, services, runtime })
}

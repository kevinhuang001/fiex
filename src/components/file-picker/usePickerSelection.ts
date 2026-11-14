import { quietProgressChannel } from '@/hooks/useActivity'
import { reportError } from '@/lib/errorCenter'
import { settleFilePicker, type FilePickerRequest } from '@/lib/filePicker'
import { call } from '@/lib/api'
import type { Entry } from '@/lib/types'
import type { Dispatch, SetStateAction } from 'react'
import type { PickerDialogState } from './PickerDialogs'
import type { PickerMenuState } from './PickerMenu'
import { join } from './types'

type PickerSelectionOptions = {
  request: FilePickerRequest
  directory: string
  entries: Entry[]
  visible: Entry[]
  selected: string[]
  setSelected: Dispatch<SetStateAction<string[]>>
  saveName: string
  setSaveName: Dispatch<SetStateAction<string>>
  replace: boolean
  setReplace: Dispatch<SetStateAction<boolean>>
  setDialog: Dispatch<SetStateAction<PickerDialogState | null>>
  setMenu: Dispatch<SetStateAction<PickerMenuState | null>>
  setReload: Dispatch<SetStateAction<number>>
  setBusy: Dispatch<SetStateAction<boolean>>
  setError: Dispatch<SetStateAction<string>>
  navigate: (path: string) => void
}

/** Owns selection rules, picker confirmation, and file mutation actions. */
export function usePickerSelection({
  request,
  directory,
  entries,
  visible,
  selected,
  setSelected,
  saveName,
  setSaveName,
  replace,
  setReplace,
  setDialog,
  setMenu,
  setReload,
  setBusy,
  setError,
  navigate,
}: PickerSelectionOptions) {
  const selectedEntries = visible.filter((entry) => selected.includes(entry.path))
  const canConfirm =
    request.mode === 'saveFile'
      ? Boolean(saveName.trim()) && Boolean(directory)
      : request.mode === 'openDirectory'
        ? Boolean(directory)
        : selectedEntries.length > 0
  const target = entries.find((entry) => entry.name === saveName.trim() && !entry.isDir)

  const confirm = () => {
    if (!canConfirm) return
    if (request.mode === 'saveFile') {
      if (target && !replace) {
        setReplace(true)
        return
      }
      return settleFilePicker(join(directory, saveName.trim()))
    }
    if (request.mode === 'openDirectory') {
      const folder = selectedEntries.find((entry) => entry.isDir)
      return settleFilePicker(folder ? folder.path : directory)
    }
    const files = selectedEntries.filter((entry) => !entry.isDir)
    if (!files.length) return
    settleFilePicker(
      request.mode === 'openFiles' ? files.map((entry) => entry.path) : files[0].path,
    )
  }

  const openEntry = (entry: Entry) => {
    if (entry.isDir) return navigate(entry.path)
    if (request.mode === 'saveFile') {
      setSaveName(entry.name)
      setSelected([entry.path])
      return
    }
    settleFilePicker(request.mode === 'openFiles' ? [entry.path] : entry.path)
  }

  const toggle = (entry: Entry, multi: boolean) => {
    if (multi || request.mode === 'openFiles') {
      setSelected((old) =>
        old.includes(entry.path) ? old.filter((path) => path !== entry.path) : [...old, entry.path],
      )
    } else {
      setSelected([entry.path])
      if (request.mode === 'saveFile') setSaveName(entry.name)
    }
  }

  const perform = async (action: () => Promise<unknown>) => {
    setBusy(true)
    setError('')
    try {
      await action()
      setDialog(null)
      setMenu(null)
      setReload((value) => value + 1)
    } catch (cause) {
      setError(String(cause))
      reportError(cause, 'file-picker')
    } finally {
      setBusy(false)
    }
  }

  const createFolder = (name: string) => {
    if (!name.trim()) return
    void perform(async () => {
      const path = await call('create_entry', { path: directory, name: name.trim() })
      setSelected([path])
    })
  }
  const renameEntry = (entry: Entry, name: string) => {
    if (!name.trim() || name.trim() === entry.name) return
    void perform(() => call('rename_entry', { path: entry.path, name: name.trim() }))
  }
  const trashEntry = (entry: Entry) => {
    void perform(() => call('recycle', { paths: [entry.path], onProgress: quietProgressChannel() }))
  }

  return {
    selectedEntries,
    canConfirm,
    confirm,
    openEntry,
    toggle,
    createFolder,
    renameEntry,
    trashEntry,
    confirmReplace: () => settleFilePicker(join(directory, saveName.trim())),
    cancelReplace: () => setReplace(false),
    cancel: () => settleFilePicker(null),
  }
}

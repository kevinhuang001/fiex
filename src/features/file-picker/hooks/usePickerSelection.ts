import type { FilePickerRequest } from '@/lib/filePicker'
import type { Entry } from '@/lib/types'
import type { Dispatch, SetStateAction } from 'react'
import type { PickerDialogState, PickerMenuState, PickerOperationServices } from '../types'
import { join } from '../types'
import {
  canConfirmPickerSelection,
  pickerConfirmationDecision,
  selectedPickerEntries,
  togglePickerSelection,
} from '../selectionModel'

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
  services: PickerOperationServices
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
  services,
}: PickerSelectionOptions) {
  const selectedEntries = selectedPickerEntries(visible, selected)
  const canConfirm = canConfirmPickerSelection(request, directory, saveName, selectedEntries)
  const target = entries.find((entry) => entry.name === saveName.trim() && !entry.isDir)

  const confirm = () => {
    const decision = pickerConfirmationDecision({
      request,
      directory,
      saveName,
      selectedEntries,
      targetExists: Boolean(target),
      replace,
    })
    if (decision.kind === 'replace') setReplace(true)
    else if (decision.kind === 'settle') services.complete(decision.value)
  }

  const openEntry = (entry: Entry) => {
    if (entry.isDir) return navigate(entry.path)
    if (request.mode === 'saveFile') {
      setSaveName(entry.name)
      setSelected([entry.path])
      return
    }
    services.complete(request.mode === 'openFiles' ? [entry.path] : entry.path)
  }

  const toggle = (entry: Entry, multi: boolean) => {
    setSelected((old) => togglePickerSelection(old, entry, multi, request.mode))
    if (!multi && request.mode === 'saveFile') setSaveName(entry.name)
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
      services.reportError(cause, 'file-picker')
    } finally {
      setBusy(false)
    }
  }

  const createFolder = (name: string) => {
    if (!name.trim()) return
    void perform(async () => {
      const path = await services.createFolder(directory, name.trim())
      setSelected([path])
    })
  }
  const renameEntry = (entry: Entry, name: string) => {
    if (!name.trim() || name.trim() === entry.name) return
    void perform(() => services.rename(entry.path, name.trim()))
  }
  const trashEntry = (entry: Entry) => {
    void perform(() => services.recycle(entry.path))
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
    confirmReplace: () => services.complete(join(directory, saveName.trim())),
    cancelReplace: () => setReplace(false),
    cancel: () => services.complete(null),
  }
}

import type { FilePickerRequest } from '@/lib/filePicker'
import type { Entry } from '@/lib/types'
import { join } from './types'

export type PickerConfirmationDecision =
  { kind: 'none' } | { kind: 'replace' } | { kind: 'settle'; value: string | string[] }

export function selectedPickerEntries(visible: Entry[], selectedPaths: string[]): Entry[] {
  const selected = new Set(selectedPaths)
  return visible.filter((entry) => selected.has(entry.path))
}

export function canConfirmPickerSelection(
  request: FilePickerRequest,
  directory: string,
  saveName: string,
  selectedEntries: Entry[],
): boolean {
  if (request.mode === 'saveFile') return Boolean(saveName.trim()) && Boolean(directory)
  if (request.mode === 'openDirectory') return Boolean(directory)
  return selectedEntries.some((entry) => !entry.isDir)
}

export function togglePickerSelection(
  selectedPaths: string[],
  entry: Entry,
  multi: boolean,
  mode: FilePickerRequest['mode'],
): string[] {
  if (multi || mode === 'openFiles') {
    return selectedPaths.includes(entry.path)
      ? selectedPaths.filter((path) => path !== entry.path)
      : [...selectedPaths, entry.path]
  }
  return [entry.path]
}

export function pickerConfirmationDecision(input: {
  request: FilePickerRequest
  directory: string
  saveName: string
  selectedEntries: Entry[]
  targetExists: boolean
  replace: boolean
}): PickerConfirmationDecision {
  const { request, directory, saveName, selectedEntries, targetExists, replace } = input
  if (!canConfirmPickerSelection(request, directory, saveName, selectedEntries))
    return { kind: 'none' }

  if (request.mode === 'saveFile') {
    if (targetExists && !replace) return { kind: 'replace' }
    return { kind: 'settle', value: join(directory, saveName.trim()) }
  }
  if (request.mode === 'openDirectory') {
    return {
      kind: 'settle',
      value: selectedEntries.find((entry) => entry.isDir)?.path || directory,
    }
  }

  const files = selectedEntries.filter((entry) => !entry.isDir)
  if (request.mode === 'openFiles')
    return { kind: 'settle', value: files.map((entry) => entry.path) }
  const firstFile = files[0]
  return firstFile ? { kind: 'settle', value: firstFile.path } : { kind: 'none' }
}

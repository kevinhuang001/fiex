import { describe, expect, it } from 'vitest'
import type { FilePickerRequest } from '@/lib/filePicker'
import type { Entry } from '@/lib/types'
import {
  canConfirmPickerSelection,
  pickerConfirmationDecision,
  selectedPickerEntries,
  togglePickerSelection,
} from './selectionModel'

function entry(path: string, isDir = false): Entry {
  return {
    path,
    name: path.split('/').at(-1) || path,
    isDir,
    isSymlink: false,
    size: 1,
    modified: 0,
    created: 0,
    extension: '',
    mime: '',
    linkTarget: null,
    tags: [],
    sync: null,
  }
}

const file = entry('/docs/a.txt')
const secondFile = entry('/docs/b.txt')
const folder = entry('/docs/subfolder', true)

describe('file-picker selection model', () => {
  it('returns selected visible entries in listing order', () => {
    expect(selectedPickerEntries([secondFile, folder, file], [file.path, folder.path])).toEqual([
      folder,
      file,
    ])
  })

  it('does not enable file confirmation for a directory-only selection', () => {
    expect(canConfirmPickerSelection({ mode: 'openFile' }, '/docs', '', [folder])).toBe(false)
    expect(canConfirmPickerSelection({ mode: 'openFiles' }, '/docs', '', [folder])).toBe(false)
    expect(canConfirmPickerSelection({ mode: 'openDirectory' }, '/docs', '', [folder])).toBe(true)
  })

  it('uses single selection except when the request supports multiple files', () => {
    expect(togglePickerSelection([file.path], secondFile, false, 'openFile')).toEqual([
      secondFile.path,
    ])
    expect(togglePickerSelection([file.path], secondFile, false, 'openFiles')).toEqual([
      file.path,
      secondFile.path,
    ])
    expect(togglePickerSelection([file.path, secondFile.path], file, false, 'openFiles')).toEqual([
      secondFile.path,
    ])
  })

  it('prompts before replacing an existing save target', () => {
    const decision = pickerConfirmationDecision({
      request: { mode: 'saveFile' },
      directory: '/docs',
      saveName: ' report.txt ',
      selectedEntries: [],
      targetExists: true,
      replace: false,
    })

    expect(decision).toEqual({ kind: 'replace' })
  })

  it('returns the requested save path after replacement is confirmed', () => {
    expect(
      pickerConfirmationDecision({
        request: { mode: 'saveFile' },
        directory: '/docs',
        saveName: ' report.txt ',
        selectedEntries: [],
        targetExists: true,
        replace: true,
      }),
    ).toEqual({ kind: 'settle', value: '/docs/report.txt' })
  })

  it('returns the current directory or the selected folder for directory requests', () => {
    const request: FilePickerRequest = { mode: 'openDirectory' }
    expect(
      pickerConfirmationDecision({
        request,
        directory: '/docs',
        saveName: '',
        selectedEntries: [],
        targetExists: false,
        replace: false,
      }),
    ).toEqual({ kind: 'settle', value: '/docs' })
    expect(
      pickerConfirmationDecision({
        request,
        directory: '/docs',
        saveName: '',
        selectedEntries: [folder],
        targetExists: false,
        replace: false,
      }),
    ).toEqual({ kind: 'settle', value: folder.path })
  })

  it('returns only files in single-file and multi-file modes', () => {
    const selectedEntries = [folder, file, secondFile]
    const base = {
      directory: '/docs',
      saveName: '',
      selectedEntries,
      targetExists: false,
      replace: false,
    }
    expect(pickerConfirmationDecision({ ...base, request: { mode: 'openFile' } })).toEqual({
      kind: 'settle',
      value: file.path,
    })
    expect(pickerConfirmationDecision({ ...base, request: { mode: 'openFiles' } })).toEqual({
      kind: 'settle',
      value: [file.path, secondFile.path],
    })
  })

  it('rejects confirmation when the mode has no valid selection or save name', () => {
    expect(
      pickerConfirmationDecision({
        request: { mode: 'openFile' },
        directory: '/docs',
        saveName: '',
        selectedEntries: [folder],
        targetExists: false,
        replace: false,
      }),
    ).toEqual({ kind: 'none' })
    expect(
      pickerConfirmationDecision({
        request: { mode: 'saveFile' },
        directory: '/docs',
        saveName: '  ',
        selectedEntries: [],
        targetExists: false,
        replace: false,
      }),
    ).toEqual({ kind: 'none' })
  })
})

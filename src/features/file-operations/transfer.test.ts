import type { Channel } from '@tauri-apps/api/core'
import type { OperationProgress } from '@/lib/types'
import { describe, expect, it, vi } from 'vitest'
import { transferActions } from './transfer'
import type { FileOpsContext, FileOperationServices } from './types'

describe('transferActions', () => {
  it('uses injected provider services and carries source directories into the transfer', async () => {
    const progress = {} as Channel<OperationProgress>
    const services = {
      fileInfo: vi.fn(),
      createEntry: vi.fn(),
      createFileEntry: vi.fn(),
      recycle: vi.fn(),
      deletePermanently: vi.fn(),
      emptyTrash: vi.fn(),
      trashRestoreNeedsFolder: vi.fn(),
      restoreFromTrash: vi.fn(),
      undo: vi.fn(),
      pickDirectory: vi.fn(),
      fileParent: vi.fn(async () => '/source'),
      transfer: vi.fn(async () => ({ paths: ['/destination/file.txt'], errors: [], skipped: 0 })),
      reportPermissionError: vi.fn(() => false),
    } as unknown as FileOperationServices
    const run = vi.fn(async (_label: string, operation: () => Promise<unknown>) => operation())
    const setClipboard = vi.fn()
    const setConflict = vi.fn()
    const actions = transferActions({
      x: {
        path: '/destination',
        query: '',
        entries: [],
        selected: ['/source/file.txt'],
        directoryAllowedOps: ['write'],
        setError: vi.fn(),
        setQuery: vi.fn(),
        setSelected: vi.fn(),
        beginRename: vi.fn(),
      },
      conflict: null,
      setConflict,
      setClipboard,
      run,
      activity: {
        progressChannel: vi.fn(() => progress),
        quietProgressChannel: vi.fn(() => progress),
        updateActivity: vi.fn(),
      },
      services,
    } as unknown as FileOpsContext)

    await actions.executeTransfer(['/source/file.txt'], '/destination', true)

    expect(services.fileParent).toHaveBeenCalledWith('/source/file.txt')
    expect(services.transfer).toHaveBeenCalledWith(
      expect.objectContaining({
        paths: ['/source/file.txt'],
        destination: '/destination',
        moving: true,
        keepBoth: false,
        ignores: [],
        resolutions: [],
      }),
    )
    expect(setConflict).toHaveBeenCalledWith(null)
    expect(setClipboard).toHaveBeenCalledWith(null)
  })
})

import { afterEach, describe, expect, it, vi } from 'vitest'

import type { FileOpsContext } from './types'
import { restoreActions } from './restore'

describe('trash restore fallback selection', () => {
  afterEach(() => vi.resetAllMocks())

  it('uses Fiex’s directory picker when no original location is available', async () => {
    const paths = ['trash://item']
    const notify = vi.fn()
    const setSelected = vi.fn()
    const run = vi.fn(async (_label: string, operation: () => Promise<unknown>) => operation())
    const services = {
      fileInfo: vi.fn(),
      createEntry: vi.fn(),
      createFileEntry: vi.fn(),
      recycle: vi.fn(),
      deletePermanently: vi.fn(),
      emptyTrash: vi.fn(),
      trashRestoreNeedsFolder: vi.fn(async () => true),
      restoreFromTrash: vi.fn(async () => 1),
      undo: vi.fn(),
      pickDirectory: vi.fn(async () => '/Users/kevin/Downloads'),
      reportPermissionError: vi.fn(() => false),
    }

    const actions = restoreActions({
      x: { path: 'trash://', setSelected } as unknown as FileOpsContext['x'],
      notify,
      run,
      services,
    } as unknown as FileOpsContext)

    actions.restoreFromTrash(paths)
    await vi.waitFor(() =>
      expect(services.restoreFromTrash).toHaveBeenCalledWith(paths, '/Users/kevin/Downloads'),
    )

    expect(services.pickDirectory).toHaveBeenCalledOnce()
    expect(notify).toHaveBeenCalled()
    expect(setSelected).toHaveBeenCalledWith([])
  })
})

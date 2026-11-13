import { afterEach, describe, expect, it, vi } from 'vitest'

const { call, openFilePicker } = vi.hoisted(() => ({
  call: vi.fn(),
  openFilePicker: vi.fn(),
}))

vi.mock('@/lib/api', () => ({ call }))
vi.mock('@/lib/filePicker', () => ({ openFilePicker }))

import { call as apiCall } from '@/lib/api'
import { openFilePicker as pickFromApp } from '@/lib/filePicker'
import type { FileOpsContext } from './types'
import { restoreActions } from './restore'

describe('trash restore fallback selection', () => {
  afterEach(() => vi.resetAllMocks())

  it('uses Fiex’s directory picker when no original location is available', async () => {
    const paths = ['trash://item']
    const notify = vi.fn()
    const setSelected = vi.fn()
    const run = vi.fn(async (_label: string, operation: () => Promise<unknown>) => operation())
    vi.mocked(apiCall).mockImplementation(async (command) => {
      if (command === 'trash_restore_needs_folder') return true
      if (command === 'restore_from_trash') return 1
      return undefined
    })
    vi.mocked(pickFromApp).mockResolvedValue('/Users/kevin/Downloads')

    const actions = restoreActions({
      x: { path: 'trash://', setSelected } as unknown as FileOpsContext['x'],
      notify,
      run,
    } as unknown as FileOpsContext)

    actions.restoreFromTrash(paths)
    await vi.waitFor(() =>
      expect(apiCall).toHaveBeenCalledWith('restore_from_trash', expect.anything()),
    )

    expect(pickFromApp).toHaveBeenCalledWith({ mode: 'openDirectory' })
    expect(apiCall).not.toHaveBeenCalledWith('pick_folder', {})
    expect(apiCall).toHaveBeenCalledWith('restore_from_trash', {
      paths,
      fallback: '/Users/kevin/Downloads',
    })
    expect(notify).toHaveBeenCalled()
    expect(setSelected).toHaveBeenCalledWith([])
  })
})

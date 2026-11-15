import type { Entry } from '@/lib/types'
import { describe, expect, it, vi } from 'vitest'
import { renameSingleEntry } from './rename'

const entry = (allowedOps?: string[]) => ({ path: '/docs/draft.txt', allowedOps }) as Entry

describe('renameSingleEntry', () => {
  it('does not invoke rename when the selected entry lacks permission', async () => {
    const invoke = vi.fn(async () => '/docs/renamed.txt')

    await expect(
      renameSingleEntry('/docs/draft.txt', 'renamed.txt', [entry(['read'])], invoke),
    ).resolves.toEqual({ status: 'unsupported' })
    expect(invoke).not.toHaveBeenCalled()
  })

  it('renames an entry and returns the provider path', async () => {
    const invoke = vi.fn(async () => '/docs/renamed.txt')

    await expect(
      renameSingleEntry('/docs/draft.txt', 'renamed.txt', [entry(['rename'])], invoke),
    ).resolves.toEqual({ status: 'renamed', path: '/docs/renamed.txt' })
    expect(invoke).toHaveBeenCalledWith('/docs/draft.txt', 'renamed.txt')
  })

  it('turns provider errors into a failure result', async () => {
    const invoke = vi.fn(async () => {
      throw new Error('destination exists')
    })

    await expect(
      renameSingleEntry('/docs/draft.txt', 'taken.txt', [entry(['rename'])], invoke),
    ).resolves.toEqual({ status: 'failed', message: 'Error: destination exists' })
  })
})

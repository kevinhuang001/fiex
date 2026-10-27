import { describe, expect, it, vi } from 'vitest'

const { call } = vi.hoisted(() => ({ call: vi.fn() }))
vi.mock('./api', () => ({ call }))
import { listDirectory } from './listing'

describe('listDirectory', () => {
  it('shares concurrent IPC but fetches again after completion', async () => {
    let finish!: (value: unknown) => void
    call.mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)))
    const first = listDirectory('/example', false, [])
    const same = listDirectory('/example', false, [])
    expect(first).toBe(same)
    expect(call).toHaveBeenCalledTimes(1)
    finish({ path: '/example', entries: [] })
    await first
    call.mockResolvedValueOnce({ path: '/example', entries: [1] })
    expect((await listDirectory('/example', false, [])).entries).toEqual([1])
    expect(call).toHaveBeenCalledTimes(2)
  })
})

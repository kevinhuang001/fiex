import { describe, expect, it, vi } from 'vitest'

const { call } = vi.hoisted(() => ({ call: vi.fn() }))
vi.mock('./api', () => ({ call }))
import { listDirectory, parentDirectoryOf } from './listing'

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

describe('parentDirectoryOf', () => {
  it.each([
    ['/home/user/file.txt', '/home/user'],
    ['C:\\Users\\user\\file.txt', 'C:\\Users\\user'],
    ['applications://Root/Note', 'applications://Root'],
    ['trash://Folder/Note', 'trash://Folder'],
    ['recyclebin://Note?id=opaque-id', 'recyclebin://'],
    ['trash://opaque-child', 'trash://'],
    ['fiex://mount/folder/item', 'fiex://mount/folder'],
  ])('resolves the parent of %s', (path, parent) => {
    expect(parentDirectoryOf(path)).toBe(parent)
  })
})

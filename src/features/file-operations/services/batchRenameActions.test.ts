import { describe, expect, it, vi } from 'vitest'
import type { Entry } from '@/lib/types'
import { createBatchRenameActions } from './batchRenameActions'

function setup() {
  const setSelected = vi.fn()
  const reload = vi.fn()
  const setBatchRename = vi.fn()
  const notify = vi.fn()
  const renameEntry = vi.fn(async (path: string, name: string) => {
    const parent = path.slice(0, path.lastIndexOf('/'))
    return `${parent}/${name}`
  })
  const x = {
    selected: ['/docs/a.txt', '/docs/b.txt'],
    entries: [
      { path: '/docs/a.txt', name: 'a.txt' },
      { path: '/docs/b.txt', name: 'b.txt' },
      { path: '/docs/c.txt', name: 'c.txt' },
    ] as Entry[],
    setSelected,
    reload,
  }
  return {
    actions: createBatchRenameActions({ x, services: { renameEntry }, notify, setBatchRename }),
    setSelected,
    reload,
    setBatchRename,
    notify,
    renameEntry,
  }
}

describe('createBatchRenameActions', () => {
  it('opens the dialog only when the requested set has multiple visible entries', () => {
    const { actions, setBatchRename } = setup()

    actions.openBatchRename()
    actions.openBatchRename(['/docs/a.txt'])
    actions.openBatchRename(['/docs/a.txt', '/docs/c.txt'])

    expect(setBatchRename).toHaveBeenCalledTimes(2)
    expect(setBatchRename).toHaveBeenNthCalledWith(1, [
      { path: '/docs/a.txt', name: 'a.txt' },
      { path: '/docs/b.txt', name: 'b.txt' },
    ])
    expect(setBatchRename).toHaveBeenNthCalledWith(2, [
      { path: '/docs/a.txt', name: 'a.txt' },
      { path: '/docs/c.txt', name: 'c.txt' },
    ])
  })

  it('applies every rename, clears selection, reloads, and reports completion', async () => {
    const { actions, setBatchRename, setSelected, reload, notify, renameEntry } = setup()

    await actions.applyBatchRename([
      { path: '/docs/a.txt', name: 'first.txt' },
      { path: '/docs/b.txt', name: 'second.txt' },
    ])

    expect(renameEntry).toHaveBeenCalledTimes(4)
    expect(setBatchRename).toHaveBeenCalledWith(null)
    expect(setSelected).toHaveBeenCalledWith([])
    expect(reload).toHaveBeenCalledOnce()
    expect(notify).toHaveBeenCalledWith('已重命名 2 个项目')
  })
})

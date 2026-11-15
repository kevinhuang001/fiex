import { describe, expect, it } from 'vitest'
import { batchRenameEntries } from './batchRename'

function renameStore(initial: Record<string, string>) {
  const files = new Map(Object.entries(initial))
  const calls: Array<[string, string]> = []
  const renameEntry = async (path: string, name: string) => {
    calls.push([path, name])
    const parent = path.slice(0, path.lastIndexOf('/'))
    const destination = `${parent}/${name}`
    if (files.has(destination)) throw new Error(`destination exists: ${destination}`)
    const contents = files.get(path)
    if (contents === undefined) throw new Error(`missing source: ${path}`)
    files.delete(path)
    files.set(destination, contents)
    return destination
  }
  return { files, calls, renameEntry }
}

describe('batchRenameEntries', () => {
  it('stages every source before final names to support swaps', async () => {
    const store = renameStore({ '/dir/a.txt': 'A', '/dir/b.txt': 'B' })
    const paths = await batchRenameEntries(
      [
        { path: '/dir/a.txt', name: 'b.txt' },
        { path: '/dir/b.txt', name: 'a.txt' },
      ],
      store.renameEntry,
      (index) => `.temp-${index}`,
    )

    expect(store.calls.slice(0, 2)).toEqual([
      ['/dir/a.txt', '.temp-0'],
      ['/dir/b.txt', '.temp-1'],
    ])
    expect(paths).toEqual(['/dir/b.txt', '/dir/a.txt'])
    expect(store.files).toEqual(
      new Map([
        ['/dir/b.txt', 'A'],
        ['/dir/a.txt', 'B'],
      ]),
    )
  })

  it('restores original names when final renaming fails', async () => {
    const store = renameStore({ '/dir/original.txt': 'content' })
    const renameEntry = async (path: string, name: string) => {
      if (name === 'new.txt') throw new Error('rename failed')
      return store.renameEntry(path, name)
    }

    await expect(
      batchRenameEntries(
        [{ path: '/dir/original.txt', name: 'new.txt' }],
        renameEntry,
        () => '.temp-0',
      ),
    ).rejects.toThrow('rename failed')
    expect(store.files).toEqual(new Map([['/dir/original.txt', 'content']]))
  })
})

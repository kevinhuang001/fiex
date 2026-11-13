import { describe, expect, it } from 'vitest'
import { parentDirectoryOf } from './path'

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

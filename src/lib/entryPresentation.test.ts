import { describe, expect, it } from 'vitest'
import { cachePresentationPatch, cachePresentations, cachedPresentation } from './entryPresentation'
import type { Entry, EntryDecoration } from './types'

function entry(path: string): Entry {
  return {
    path,
    name: path,
    isDir: false,
    isSymlink: false,
    size: 0,
    modified: 1,
    created: 0,
    extension: '',
    mime: 'application/octet-stream',
    linkTarget: null,
    tags: [],
    sync: null,
  }
}

describe('presentation decoration patches', () => {
  it('updates resolved rows and keeps the other cached rows', () => {
    const rows = [entry('vfs://presentation-patch-a'), entry('vfs://presentation-patch-b')]
    const decoration: EntryDecoration = {
      path: rows[0].path,
      label: 'Resolved label',
    }

    cachePresentations(rows, [])
    cachePresentationPatch(rows, [decoration])

    expect(cachedPresentation(rows[0])).toEqual(decoration)
    expect(cachedPresentation(rows[1])).toEqual({ path: rows[1].path })
  })
})

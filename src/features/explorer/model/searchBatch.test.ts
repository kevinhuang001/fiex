import { describe, expect, it } from 'vitest'
import type { Batch, Entry } from '@/lib/types'
import { mergeSearchBatch } from './searchBatch'

function entry(path: string, name = path): Entry {
  return {
    path,
    name,
    isDir: false,
    isSymlink: false,
    size: 0,
    modified: 0,
    created: 0,
    extension: '',
    mime: '',
    linkTarget: null,
    tags: [],
    sync: null,
  }
}

function batch(entries: Entry[], replace = false): Batch {
  return { entries, replace, done: false, elapsedMs: 0, truncated: false, skipped: 0 }
}

describe('mergeSearchBatch', () => {
  it('replaces virtual-folder results as a stable snapshot', () => {
    const existing = [entry('/a')]
    expect(mergeSearchBatch(existing, batch([entry('/a')], true))).toBe(existing)
    expect(mergeSearchBatch(existing, batch([entry('/b')], true))).toEqual([entry('/b')])
  })

  it('merges streamed entries by path and retains unchanged object identity', () => {
    const original = entry('/a')
    const previous = [original]
    expect(mergeSearchBatch(previous, batch([original]))).toBe(previous)
    expect(mergeSearchBatch(previous, batch([entry('/b')]))).toEqual([original, entry('/b')])
    expect(mergeSearchBatch(previous, batch([entry('/a', 'renamed')]))).toEqual([
      entry('/a', 'renamed'),
    ])
  })
})

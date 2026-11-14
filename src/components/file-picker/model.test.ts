import { describe, expect, it } from 'vitest'
import { groupPickerEntries, visiblePickerEntries } from './model'
import type { Entry } from '@/lib/types'

function entry(
  name: string,
  options: Partial<Pick<Entry, 'isDir' | 'size' | 'modified' | 'extension'>> = {},
): Entry {
  return {
    path: `/${name}`,
    name,
    isDir: false,
    isSymlink: false,
    size: 1,
    modified: 0,
    created: 0,
    extension: name.includes('.') ? name.split('.').at(-1) || '' : '',
    mime: '',
    linkTarget: null,
    tags: [],
    sync: null,
    ...options,
  }
}

const sort = { key: 'name' as const, direction: 'asc' as const }

describe('visiblePickerEntries', () => {
  it('filters names without hiding directories or matching extensions case-sensitively', () => {
    const entries = [
      entry('notes.TXT'),
      entry('image.png'),
      entry('documents', { isDir: true }),
      entry('Readme.txt'),
    ]

    expect(
      visiblePickerEntries(entries, 'read', 'openFiles', ['.txt'], sort, 'none').map(
        (item) => item.name,
      ),
    ).toEqual(['Readme.txt'])
    expect(
      visiblePickerEntries(entries, '', 'openFiles', ['txt'], sort, 'none').map(
        (item) => item.name,
      ),
    ).toEqual(['documents', 'notes.TXT', 'Readme.txt'])
  })

  it('keeps all file extensions in save mode and sorts directories first', () => {
    const entries = [entry('z.png'), entry('a.txt'), entry('folder', { isDir: true })]

    expect(
      visiblePickerEntries(entries, '', 'saveFile', ['.txt'], sort, 'none').map(
        (item) => item.name,
      ),
    ).toEqual(['folder', 'a.txt', 'z.png'])
  })

  it('groups entries into date buckets using the supplied clock', () => {
    const now = 10 * 24 * 60 * 60 * 1000
    const entries = [
      entry('older', { modified: 0 }),
      entry('today', { modified: 10 * 24 * 60 * 60 }),
      entry('yesterday', { modified: 9 * 24 * 60 * 60 }),
    ]
    const result = visiblePickerEntries(entries, '', 'openFiles', undefined, sort, 'modified', now)

    expect(result.map((item) => item.name)).toEqual(['today', 'yesterday', 'older'])
    expect(
      groupPickerEntries(result, 'modified', now).map((section) => section.entries[0].name),
    ).toEqual(['today', 'yesterday', 'older'])
  })

  it('groups size ranges in rank order and keeps same-range files together', () => {
    const entries = [
      entry('medium.bin', { size: 2 * 1024 * 1024 }),
      entry('empty.bin', { size: 0 }),
      entry('tiny.bin', { size: 4 }),
      entry('folder', { isDir: true }),
      entry('small.bin', { size: 20 }),
    ]
    const result = visiblePickerEntries(entries, '', 'openFiles', undefined, sort, 'size', 0)

    expect(result.map((item) => item.name)).toEqual([
      'folder',
      'empty.bin',
      'small.bin',
      'tiny.bin',
      'medium.bin',
    ])
    expect(groupPickerEntries(result, 'size', 0).map((section) => section.entries.length)).toEqual([
      1, 1, 2, 1,
    ])
  })
})

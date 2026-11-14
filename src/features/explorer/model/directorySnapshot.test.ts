import { describe, expect, it } from 'vitest'
import { reconcileDirectorySnapshot } from './directorySnapshot'
import type { Entry, EntryDecoration, Listing } from '@/lib/types'

function entry(path: string, name: string): Entry {
  return {
    path,
    name,
    isDir: false,
    isSymlink: false,
    size: 10,
    modified: 1,
    created: 1,
    extension: 'txt',
    mime: 'text/plain',
    linkTarget: null,
    tags: [],
    sync: null,
  }
}

function listing(entries: Entry[], warnings: string[] = []): Listing {
  return { path: '/docs', entries, warnings, allowedOps: [] }
}

describe('reconcileDirectorySnapshot', () => {
  it('preserves the visible entry array and listing object while advancing revision', () => {
    const first = entry('/docs/a.txt', 'a.txt')
    const second = entry('/docs/b.txt', 'b.txt')
    const previous = {
      listing: listing([first, second]),
      decorations: [] as EntryDecoration[],
      revision: 4,
    }

    const result = reconcileDirectorySnapshot(
      previous,
      listing([second, first]),
      [{ path: first.path, label: 'updated' }],
      5,
      'name',
      false,
    )

    expect(result.visibleChanged).toBe(false)
    expect(result.entries).toBe(previous.listing.entries)
    expect(result.snapshot.listing).toBe(previous.listing)
    expect(result.snapshot.decorations).toEqual([{ path: first.path, label: 'updated' }])
    expect(result.snapshot.revision).toBe(5)
  })

  it('publishes changed visible entries while retaining the newest metadata', () => {
    const previous = {
      listing: listing([entry('/docs/a.txt', 'a.txt')]),
      decorations: [] as EntryDecoration[],
      revision: 4,
    }
    const next = listing([entry('/docs/a.txt', 'renamed.txt')], ['partial warning'])
    const result = reconcileDirectorySnapshot(previous, next, [], 5, 'name', false)

    expect(result.visibleChanged).toBe(true)
    expect(result.entries).toBe(next.entries)
    expect(result.snapshot.listing).not.toBe(previous.listing)
    expect(result.snapshot.listing.warnings).toEqual(['partial warning'])
  })

  it('refreshes metadata without replacing entries when visible content is unchanged', () => {
    const entries = [entry('/docs/a.txt', 'a.txt')]
    const previous = {
      listing: listing(entries),
      decorations: [] as EntryDecoration[],
      revision: 4,
    }
    const result = reconcileDirectorySnapshot(
      previous,
      listing([entry('/docs/a.txt', 'a.txt')], ['new warning']),
      [],
      5,
      'name',
      false,
    )

    expect(result.visibleChanged).toBe(false)
    expect(result.entries).toBe(entries)
    expect(result.snapshot.listing).not.toBe(previous.listing)
    expect(result.snapshot.listing.warnings).toEqual(['new warning'])
  })
})

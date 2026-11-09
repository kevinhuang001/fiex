import { describe, expect, it } from 'vitest'
import type { EntryDecoration, Listing } from '@/lib/types'
import {
  PaintedListingCache,
  sameDirectoryDecorations,
  sameListingMetadata,
} from './paintedListingCache'

function listing(overrides: Partial<Listing> = {}): Listing {
  return {
    allowedOps: ['list'],
    path: '/docs',
    entries: [],
    warnings: [],
    ...overrides,
  }
}

function snapshot(path: string) {
  return { listing: listing({ path }), decorations: [], revision: 1 }
}

describe('PaintedListingCache', () => {
  it('refreshes recency when a cached directory is painted again', () => {
    const cache = new PaintedListingCache(2)
    const first = snapshot('/first')
    const second = snapshot('/second')
    const third = snapshot('/third')

    cache.remember('first', first)
    cache.remember('second', second)
    expect(cache.touch('first')).toBe(first)
    cache.remember('third', third)

    expect(cache.get('first')).toBe(first)
    expect(cache.get('second')).toBeNull()
    expect(cache.get('third')).toBe(third)
  })

  it('replaces a snapshot without increasing the cache size', () => {
    const cache = new PaintedListingCache(2)
    const original = snapshot('/docs')
    const updated = { ...snapshot('/docs'), revision: 2 }
    cache.remember('docs', original)
    cache.remember('other', snapshot('/other'))
    cache.remember('docs', updated)

    expect(cache.get('docs')).toBe(updated)
    expect(cache.touch('other')).not.toBeNull()
  })

  it('rejects an invalid capacity', () => {
    expect(() => new PaintedListingCache(0)).toThrow(RangeError)
    expect(() => new PaintedListingCache(1.5)).toThrow(RangeError)
  })
})

describe('sameListingMetadata', () => {
  it('compares directory capabilities and presentation metadata', () => {
    expect(sameListingMetadata(listing(), listing({ allowedOps: ['list', 'create'] }))).toBe(false)
    expect(sameListingMetadata(listing(), listing({ path: '/other' }))).toBe(false)
    expect(sameListingMetadata(listing(), listing({ warnings: ['offline'] }))).toBe(false)
    expect(sameListingMetadata(listing(), listing({ defaultView: 'list' }))).toBe(false)
    expect(sameListingMetadata(listing(), listing({ allowedOps: ['list'] }))).toBe(true)
  })
})

describe('sameDirectoryDecorations', () => {
  it('compares decorations by resource path regardless of order', () => {
    const first: EntryDecoration = { path: '/a', label: 'A' }
    const second: EntryDecoration = { path: '/b', label: 'B' }

    expect(sameDirectoryDecorations([first, second], [second, first])).toBe(true)
    expect(sameDirectoryDecorations([first], [{ ...first, label: 'changed' }])).toBe(false)
    expect(sameDirectoryDecorations([first], [second])).toBe(false)
  })
})

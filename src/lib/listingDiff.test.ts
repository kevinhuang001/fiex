import { describe, expect, it } from 'vitest'
import type { Entry } from './types'
import { sameListing, sameStringSet, sameVisibleListing } from './listingDiff'

const entry = (path: string, patch: Partial<Entry> = {}): Entry => ({
  path,
  name: path.split('/').at(-1) || path,
  isDir: false,
  isSymlink: false,
  size: 10,
  modified: 20,
  created: 5,
  extension: 'txt',
  mime: 'text/plain',
  linkTarget: null,
  tags: [],
  sync: null,
  ...patch,
})

describe('sameListing', () => {
  it('ignores provider response ordering when entries have the same identity and data', () => {
    const first = [entry('/a'), entry('/b')]
    expect(sameListing(first, [...first].reverse())).toBe(true)
  })

  it('detects changed entry data and duplicate identities', () => {
    expect(sameListing([entry('/a')], [entry('/a', { size: 11 })])).toBe(false)
    expect(sameListing([entry('/a')], [entry('/a'), entry('/a')])).toBe(false)
  })

  it('compares operation capabilities as a set', () => {
    expect(sameStringSet(['read', 'list'], ['list', 'read'])).toBe(true)
    expect(sameStringSet(['read'], ['read', 'write'])).toBe(false)
  })
})

describe('sameVisibleListing', () => {
  it('preserves the Entry array when response order does not change the sorted view', () => {
    const first = [entry('/b', { name: 'B' }), entry('/a', { name: 'A' })]
    expect(sameVisibleListing(first, [...first].reverse(), 'name', false)).toBe(true)
  })

  it('detects a provider-order change among entries tied by the active sort', () => {
    const first = [entry('/first', { name: 'Same' }), entry('/second', { name: 'Same' })]
    expect(sameVisibleListing(first, [...first].reverse(), 'name', false)).toBe(false)
  })
})

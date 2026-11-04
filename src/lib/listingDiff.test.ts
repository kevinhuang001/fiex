import { describe, expect, it } from 'vitest'
import type { Entry } from './types'
import { sameListing, sameStringSet } from './listingDiff'

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

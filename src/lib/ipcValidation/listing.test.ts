import { describe, expect, it } from 'vitest'
import { parseListing } from './entries'

const listing = {
  allowedOps: ['list', 'search'],
  virtualVolume: false,
  path: '/workspace',
  entries: [],
  warnings: [],
}

describe('directory listing IPC validation', () => {
  it('constructs a listing when Rust omits its nullable default view', () => {
    expect(parseListing(listing)).toEqual(listing)
  })

  it.each(['list', 'gallery', 'columns', 'icons'] as const)(
    'accepts the %s default view',
    (defaultView) => {
      expect(parseListing({ ...listing, defaultView })).toMatchObject({ defaultView })
    },
  )

  it.each([
    { ...listing, unexpected: true },
    { allowedOps: listing.allowedOps, path: listing.path, entries: [], warnings: [] },
    { ...listing, defaultView: null },
    { ...listing, defaultView: 'table' },
    { ...listing, allowedOps: ['list', 1] },
  ])('rejects malformed or unexpected listing data: %j', (value) => {
    expect(() => parseListing(value)).toThrow('Invalid directory listing from IPC')
  })
})

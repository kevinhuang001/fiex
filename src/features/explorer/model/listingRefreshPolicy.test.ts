import { describe, expect, it, vi } from 'vitest'
import {
  connectionContainsPath,
  connectionCoversPath,
  directoryChangeIncludesPath,
  listingRefreshTarget,
  shouldWatchDirectory,
} from './listingRefreshPolicy'

describe('listingRefreshTarget', () => {
  it('refreshes search results only when a query contains non-whitespace text', () => {
    expect(listingRefreshTarget(' report ')).toBe('search')
    expect(listingRefreshTarget('   ')).toBe('directory')
    expect(listingRefreshTarget('')).toBe('directory')
  })
})

describe('connectionContainsPath', () => {
  it('includes a mount and its descendants but excludes prefix siblings', () => {
    expect(connectionContainsPath('/remote/share', '/remote/share')).toBe(true)
    expect(connectionContainsPath('/remote/share/folder', '/remote/share')).toBe(true)
    expect(connectionContainsPath('/remote/share-two', '/remote/share')).toBe(false)
  })

  it('handles root mounts', () => {
    expect(connectionContainsPath('/remote/share', '/')).toBe(true)
  })
})

describe('connectionCoversPath', () => {
  it('reports whether any configured connection owns the current path', () => {
    expect(connectionCoversPath('/remote/share/file.txt', [{ mountPath: '/remote/share' }])).toBe(
      true,
    )
    expect(connectionCoversPath('/local/file.txt', [])).toBe(false)
  })
})

describe('directoryChangeIncludesPath', () => {
  it('accepts only a directory list containing the current path', () => {
    expect(directoryChangeIncludesPath({ directories: ['/a', '/b'] }, '/b')).toBe(true)
    expect(directoryChangeIncludesPath({ directories: ['/a'] }, '/b')).toBe(false)
    expect(directoryChangeIncludesPath({ directories: '/b' }, '/b')).toBe(false)
    expect(directoryChangeIncludesPath(null, '/b')).toBe(false)
  })
})

describe('shouldWatchDirectory', () => {
  const isVirtualResource = vi.fn((path: string) => path.startsWith('provider://'))
  const state = { enabled: true, native: true, path: '/home/user', virtualVolume: false }

  it('watches an enabled native local directory', () => {
    expect(shouldWatchDirectory(state, isVirtualResource)).toBe(true)
  })

  it('skips inactive, web, empty, virtual, and plugin-view directories', () => {
    expect(shouldWatchDirectory({ ...state, enabled: false }, isVirtualResource)).toBe(false)
    expect(shouldWatchDirectory({ ...state, native: false }, isVirtualResource)).toBe(false)
    expect(shouldWatchDirectory({ ...state, path: '' }, isVirtualResource)).toBe(false)
    expect(shouldWatchDirectory({ ...state, virtualVolume: true }, isVirtualResource)).toBe(false)
    expect(shouldWatchDirectory({ ...state, path: 'provider://item' }, isVirtualResource)).toBe(
      false,
    )
    expect(
      shouldWatchDirectory(
        { ...state, path: 'fiex-plugin-view://catalog/item' },
        isVirtualResource,
      ),
    ).toBe(false)
  })

  it('does not query virtual-resource status for directories that cannot be watched', () => {
    isVirtualResource.mockClear()
    shouldWatchDirectory({ ...state, enabled: false }, isVirtualResource)
    expect(isVirtualResource).not.toHaveBeenCalled()
  })
})

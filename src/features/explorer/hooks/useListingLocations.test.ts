import { describe, expect, it } from 'vitest'
import type { Location } from '@/lib/types'
import { resolveInitialPath } from './useListingLocations'

const locations: Location[] = [
  {
    name: '个人文件夹',
    path: '/home/kevin',
    kind: 'folder',
    available: 0,
    total: 0,
    removable: false,
    filesystem: '',
    device: '',
    mounted: true,
    usbfs: false,
  },
  {
    name: '文稿',
    path: '/home/kevin/Documents',
    kind: 'folder',
    available: 0,
    total: 0,
    removable: false,
    filesystem: '',
    device: '',
    mounted: true,
    usbfs: false,
  },
]

describe('resolveInitialPath', () => {
  it('prefers an explicit launch override and a launch path', () => {
    expect(
      resolveInitialPath({
        startOverride: '/tmp/requested',
        launched: '/tmp/launched',
        startPage: 'home',
        locations,
      }),
    ).toBe('/tmp/requested')
    expect(resolveInitialPath({ launched: '/tmp/launched', startPage: 'recent', locations })).toBe(
      '/tmp/launched',
    )
  })

  it('uses Recent and the last successful directory when selected', () => {
    expect(resolveInitialPath({ startPage: 'recent', locations })).toBe('recent://')
    expect(resolveInitialPath({ startPage: 'last', lastPath: '/work/current', locations })).toBe(
      '/work/current',
    )
  })

  it('resolves Home and Documents from available locations with an OS fallback', () => {
    expect(resolveInitialPath({ startPage: 'home', locations })).toBe('/home/kevin')
    expect(
      resolveInitialPath({ startPage: 'documents', initialPath: '/tmp/fallback', locations }),
    ).toBe('/home/kevin/Documents')
    expect(
      resolveInitialPath({ startPage: 'home', initialPath: '/tmp/fallback', locations: [] }),
    ).toBe('/tmp/fallback')
  })
})

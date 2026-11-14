import { describe, expect, it, vi } from 'vitest'
import type { Location } from '@/lib/types'
import { resolveInitialPath, resolveInitialPathRequest } from './initialPath'

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
    expect(resolveInitialPath({ startPage: 'last', locations })).toBe('/home/kevin/Documents')
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

  it('stops startup resolution when the pane is cancelled after launch lookup', async () => {
    let cancelled = false
    const loadLastPath = vi.fn(async () => '/work/old')
    const resolve = resolveInitialPathRequest({
      startPage: 'last',
      native: true,
      isCancelled: () => cancelled,
      loadLaunchPath: async () => null,
      loadLastPath,
      loadInitialPath: async () => '/home/kevin',
      loadLocations: async () => locations,
    })
    cancelled = true

    await expect(resolve).resolves.toBeNull()
    expect(loadLastPath).not.toHaveBeenCalled()
  })

  it('falls back from failed startup lookups to the configured Documents location', async () => {
    await expect(
      resolveInitialPathRequest({
        startPage: 'last',
        native: true,
        isCancelled: () => false,
        loadLaunchPath: async () => {
          throw new Error('launch state unavailable')
        },
        loadLastPath: async () => null,
        loadInitialPath: async () => null,
        loadLocations: async () => locations,
      }),
    ).resolves.toBe('/home/kevin/Documents')
  })
})

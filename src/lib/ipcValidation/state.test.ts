import { describe, expect, it } from 'vitest'
import { parseStateGetResult } from './state'

describe('stored state IPC validation', () => {
  it('decodes layout columns and picker sorting by namespace and key', () => {
    expect(parseStateGetResult({ namespace: 'layout', key: 'columns.hash' }, { size: 20 })).toEqual(
      { name: 260, modified: 155, size: 60, kind: 108 },
    )
    expect(
      parseStateGetResult(
        { namespace: 'picker', key: 'sort' },
        { key: 'modified', direction: 'desc' },
      ),
    ).toEqual({ key: 'modified', direction: 'desc' })
    expect(() =>
      parseStateGetResult(
        { namespace: 'picker', key: 'sort' },
        { key: 'name', direction: 'asc', extra: true },
      ),
    ).toThrow('Invalid saved picker sort')
  })

  it('validates the saved preference and location shapes', () => {
    expect(
      parseStateGetResult({ namespace: 'explorer', key: 'preferences' }, { locale: 'en' }),
    ).toEqual({ locale: 'en' })
    expect(parseStateGetResult({ namespace: 'explorer', key: 'preferences' }, null)).toBeNull()
    expect(() =>
      parseStateGetResult({ namespace: 'explorer', key: 'locations' }, [{ path: '/home' }]),
    ).toThrow('Invalid saved location')
  })

  it('checks scalar picker state and nullable saved values', () => {
    expect(parseStateGetResult({ namespace: 'picker', key: 'group' }, 'modified')).toBe('modified')
    expect(parseStateGetResult({ namespace: 'picker', key: 'lastOpenDirectory' }, '/home')).toBe(
      '/home',
    )
    expect(parseStateGetResult({ namespace: 'picker', key: 'lastSaveDirectory' }, null)).toBeNull()
    expect(parseStateGetResult({ namespace: 'explorer', key: 'lastPath' }, null)).toBeNull()
    expect(() => parseStateGetResult({ namespace: 'picker', key: 'group' }, 'extension')).toThrow(
      'Invalid saved picker group',
    )
  })
})

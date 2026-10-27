import { describe, expect, it, vi } from 'vitest'
import type { Entry, EntryDecoration } from '@/lib/types'
vi.mock('@/i18n', () => ({ t: (key: string) => key }))
import { resolveDecorations } from './index'

function entry(overrides: Partial<Entry> = {}): Entry {
  return { path: '/a', name: 'a', tags: [], sync: null, readonly: false, ...overrides } as Entry
}

describe('decoration providers', () => {
  it('renders cache mode and upload outcome together without waiting for plugins', () => {
    const value = resolveDecorations([entry({ sync: 'pinned', pinned: true, uploaded: true })], {})[
      '/a'
    ]
    expect(value.badges.map((badge) => badge.text)).toEqual(['始终保留在本地', '已上传'])
    expect(value.badges[0].icon).toBe('pin')
  })

  it('does not report an uploaded copy after it has been uncached', () => {
    const value = resolveDecorations([entry({ sync: 'cloud', uploaded: true })], {})['/a']
    expect(value.badges.map((badge) => badge.text)).toEqual(['未缓存'])
  })

  it('updates connection state from the current listing without mutating source data', () => {
    const original = entry({ sync: 'pending', hasCache: true })
    expect(resolveDecorations([original], {})['/a'].badges[0].text).toBe('等待上传')
    expect(resolveDecorations([{ ...original, sync: 'conflict' }], {})['/a'].badges[0].text).toBe(
      '同步冲突',
    )
    expect(original.sync).toBe('pending')
  })

  it('merges plugins, status, read-only and tags without losing fields or consuming plugin badges', () => {
    const native = entry({
      sync: 'synced',
      hasCache: true,
      uploaded: true,
      readonly: true,
      tags: [{ name: 'Review', color: 4 }],
    })
    const plugin: EntryDecoration = {
      path: '/a',
      badges: [],
      providers: ['Git'],
      label: 'review',
      fields: {
        name: { badges: [{ text: 'M', color: 'orange' }], style: { color: 'orange' } },
        row: { style: { backgroundLight: '#fff' } },
        size: { badges: [{ text: '+', color: 'green' }] },
      },
    }
    const before = structuredClone(plugin)
    const result = resolveDecorations([native], { '/a': plugin })['/a']
    expect(result.badges.map((badge) => badge.text)).toEqual([
      'Review',
      '只读',
      '已缓存',
      '已上传',
      'M',
    ])
    expect(result.fields?.row?.style?.backgroundLight).toBe('#fff')
    expect(result.fields?.size?.badges?.[0].text).toBe('+')
    expect(result.style?.color).toBe('orange')
    expect(result.label).toBe('review')
    expect(result.providers).toEqual(['Fiex', 'Git'])
    expect(plugin).toEqual(before)
  })

  it('drops decorations for entries that are no longer visible', () => {
    expect(resolveDecorations([entry()], { '/old': { path: '/old', badges: [] } })).toEqual({})
  })

  it('keeps builtin badges when a plugin response is missing or fails', () => {
    expect(resolveDecorations([entry({ readonly: true })], {})['/a'].badges[0].text).toBe('只读')
  })

  it('merges builtin providers by priority and lets plugin styles win', () => {
    const values = resolveDecorations(
      [entry()],
      { '/a': { path: '/a', badges: [], style: { color: 'blue' } } },
      [
        {
          id: 'high',
          priority: 10,
          resolve: () => ({ path: '/a', badges: [], style: { color: 'red' } }),
        },
        {
          id: 'low',
          priority: 0,
          resolve: () => ({ path: '/a', badges: [], style: { color: 'green', fontWeight: '600' } }),
        },
      ],
    )
    expect(values['/a'].style).toEqual({ color: 'blue', fontWeight: '600' })
  })
})

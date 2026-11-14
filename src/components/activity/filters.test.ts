import { describe, expect, it } from 'vitest'
import type { ActivityItem } from '@/hooks/useActivity'
import { getActivityPluginFilters, matchesActivityFilter } from './filters'

const item = (kind: ActivityItem['kind'], pluginId?: string): ActivityItem => ({
  id: `${kind}:${pluginId || ''}`,
  kind,
  label: 'Example',
  status: 'done',
  startedAt: 1,
  ...(pluginId === undefined ? {} : { pluginId }),
})

describe('activity filters', () => {
  it('groups plugin activities and resolves their catalog names', () => {
    const filters = getActivityPluginFilters(
      [
        item('transfer'),
        item('plugin', 'z.plugin'),
        item('plugin', 'a.plugin'),
        item('plugin', 'z.plugin'),
        item('plugin'),
      ],
      [
        { id: 'z.plugin', name: 'Zeta' },
        { id: 'a.plugin', name: 'Alpha' },
      ],
    )

    expect(filters.map(({ id, label }) => [id, label])).toEqual([
      ['', '未标记插件'],
      ['a.plugin', 'Alpha'],
      ['z.plugin', 'Zeta'],
    ])
  })

  it('matches all, built-in, and exact plugin filters', () => {
    expect(matchesActivityFilter(item('transfer'), 'all')).toBe(true)
    expect(matchesActivityFilter(item('delete'), 'builtin:delete')).toBe(true)
    expect(matchesActivityFilter(item('transfer'), 'builtin:delete')).toBe(false)
    expect(matchesActivityFilter(item('plugin', 'one'), 'plugin:one')).toBe(true)
    expect(matchesActivityFilter(item('plugin', 'other'), 'plugin:one')).toBe(false)
    expect(matchesActivityFilter(item('delete'), 'plugin:')).toBe(false)
  })
})

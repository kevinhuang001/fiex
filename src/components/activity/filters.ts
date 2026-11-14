import { getLocale, t } from '@/i18n'
import type { ActivityItem, ActivityKind } from '@/hooks/useActivity'
import type { PluginInfo } from '@/lib/plugins'

export type ActivityFilter = 'all' | `builtin:${ActivityKind}` | `plugin:${string}`

export interface ActivityPluginFilter {
  id: string
  label: string
}

export const builtinFilters: { kind: ActivityKind; get label(): string }[] = [
  {
    kind: 'transfer',
    get label() {
      return t('传输')
    },
  },
  {
    kind: 'delete',
    get label() {
      return t('删除')
    },
  },
  {
    kind: 'sync',
    get label() {
      return t('同步')
    },
  },
]

export function getActivityPluginFilters(
  items: ActivityItem[],
  plugins: Pick<PluginInfo, 'id' | 'name'>[],
): ActivityPluginFilter[] {
  const names = new Map(plugins.map((plugin) => [plugin.id, plugin.name]))
  const groups = new Map<string, ActivityPluginFilter>()
  for (const item of items) {
    if (item.kind !== 'plugin') continue
    const id = item.pluginId || ''
    if (!groups.has(id)) groups.set(id, { id, label: id ? names.get(id) || id : t('未标记插件') })
  }
  return [...groups.values()].sort((a, b) => a.label.localeCompare(b.label, getLocale()))
}

export function matchesActivityFilter(item: ActivityItem, filter: ActivityFilter): boolean {
  if (filter === 'all') return true
  if (filter.startsWith('builtin:')) return item.kind === filter.slice('builtin:'.length)
  return item.kind === 'plugin' && (item.pluginId || '') === filter.slice('plugin:'.length)
}

export function getActivityFilterLabel(
  filter: ActivityFilter,
  pluginFilters: ActivityPluginFilter[],
): string {
  if (filter === 'all') return t('全部操作')
  if (filter.startsWith('builtin:'))
    return (
      builtinFilters.find((entry) => `builtin:${entry.kind}` === filter)?.label || t('操作类型')
    )
  return pluginFilters.find((entry) => `plugin:${entry.id}` === filter)?.label || t('插件')
}

import { Check } from 'lucide-react'
import { t } from '@/i18n'
import { builtinFilters, type ActivityFilter, type ActivityPluginFilter } from './filters'

interface ActivityFilterMenuProps {
  filter: ActivityFilter
  pluginFilters: ActivityPluginFilter[]
  onSelect: (filter: ActivityFilter) => void
}

export function ActivityFilterMenu({ filter, pluginFilters, onSelect }: ActivityFilterMenuProps) {
  return (
    <div className="activity-filter-menu" role="menu" aria-label={t('筛选操作')}>
      <button
        role="menuitemradio"
        aria-checked={filter === 'all'}
        className={filter === 'all' ? 'active' : ''}
        onClick={() => onSelect('all')}
      >
        <span>{t('全部操作')}</span>
        {filter === 'all' && <Check size={14} />}
      </button>
      <div className="activity-filter-group" role="group" aria-label={t('操作类型')}>
        {builtinFilters.map((entry) => {
          const value: ActivityFilter = `builtin:${entry.kind}`
          return (
            <button
              key={value}
              role="menuitemradio"
              aria-checked={filter === value}
              className={filter === value ? 'active' : ''}
              onClick={() => onSelect(value)}
            >
              <span>{entry.label}</span>
              {filter === value && <Check size={14} />}
            </button>
          )
        })}
      </div>
      {pluginFilters.length > 0 && (
        <div className="activity-filter-group" role="group" aria-label={t('插件')}>
          <span className="activity-filter-heading" aria-hidden>
            {t('插件')}
          </span>
          {pluginFilters.map((entry) => {
            const value: ActivityFilter = `plugin:${entry.id}`
            return (
              <button
                key={value}
                role="menuitemradio"
                aria-checked={filter === value}
                className={filter === value ? 'active' : ''}
                title={entry.id || undefined}
                onClick={() => onSelect(value)}
              >
                <span>{entry.label}</span>
                {filter === value && <Check size={14} />}
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}

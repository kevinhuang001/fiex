import { ActivityDetails, StatusIcon } from './activity/ActivityDetails'
import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useRef, useState } from 'react'
import { ArrowLeft, Check, ChevronRight, Clock3, Filter, X } from 'lucide-react'
import { useActivity, type ActivityKind } from '@/hooks/useActivity'
import { usePluginCatalog } from '@/lib/plugins'
import { activityEta as eta } from '@/lib/activity'
import { size } from '@/lib/files'

const time = (value: number) =>
  new Date(value).toLocaleTimeString(getLocale(), { hour: '2-digit', minute: '2-digit' })

type ActivityFilter = 'all' | `builtin:${ActivityKind}` | `plugin:${string}`

const builtinFilters: { kind: ActivityKind; get label(): string }[] = [
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

export function ActivityCenter() {
  const activity = useActivity()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [filter, setFilter] = useState<ActivityFilter>('all')
  const [filterOpen, setFilterOpen] = useState(false)
  const filterRef = useRef<HTMLDivElement>(null)
  const panel = useRef<HTMLElement>(null)
  const plugins = usePluginCatalog()
  const pluginFilters = useMemo(() => {
    const names = new Map(plugins.map((plugin) => [plugin.id, plugin.name]))
    const groups = new Map<string, { id: string; label: string }>()
    for (const item of activity.items) {
      if (item.kind !== 'plugin') continue
      const id = item.pluginId || ''
      if (!groups.has(id)) groups.set(id, { id, label: id ? names.get(id) || id : t('未标记插件') })
    }
    return [...groups.values()].sort((a, b) => a.label.localeCompare(b.label, getLocale()))
  }, [activity.items, plugins, getLocale()])
  useEffect(() => {
    if (!activity.open) {
      setSelectedId(null)
      setFilterOpen(false)
      return
    }
    const close = (event: PointerEvent) => {
      if (!panel.current?.contains(event.target as Node)) activity.setOpen(false)
    }
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') activity.setOpen(false)
    }
    document.addEventListener('pointerdown', close)
    document.addEventListener('keydown', escape)
    return () => {
      document.removeEventListener('pointerdown', close)
      document.removeEventListener('keydown', escape)
    }
  }, [activity.open])
  useEffect(() => {
    if (!filterOpen) return
    const close = (event: PointerEvent) => {
      if (!filterRef.current?.contains(event.target as Node)) setFilterOpen(false)
    }
    document.addEventListener('pointerdown', close)
    return () => document.removeEventListener('pointerdown', close)
  }, [filterOpen])
  if (!activity.open) return null
  const selected = activity.items.find((item) => item.id === selectedId)
  const filtered = activity.items.filter((item) => {
    if (filter === 'all') return true
    if (filter.startsWith('builtin:')) return item.kind === filter.slice('builtin:'.length)
    if (item.kind !== 'plugin') return false
    return (item.pluginId || '') === filter.slice('plugin:'.length)
  })
  const running = filtered.filter((item) => item.status === 'running').length
  const activeFilterLabel =
    filter === 'all'
      ? t('全部操作')
      : filter.startsWith('builtin:')
        ? builtinFilters.find((entry) => `builtin:${entry.kind}` === filter)?.label || t('操作类型')
        : pluginFilters.find((entry) => `plugin:${entry.id}` === filter)?.label || t('插件')
  return (
    <aside
      ref={panel}
      className={`activity-center ${filterOpen ? 'filter-open' : ''}`}
      aria-label={t('状态中心')}
    >
      <header>
        {selected && (
          <button
            className="icon-button"
            aria-label={t('返回操作列表')}
            onClick={() => setSelectedId(null)}
          >
            <ArrowLeft size={17} />
          </button>
        )}
        <strong>{selected ? t('操作详情') : t('状态中心')}</strong>
        {!selected && running > 0 && (
          <span className="activity-running">
            {running} {t('进行中')}
          </span>
        )}
        <div className="activity-header-actions">
          {!selected && (
            <div className="activity-filter" ref={filterRef}>
              <button
                className={`icon-button activity-filter-button ${filter === 'all' ? '' : 'active'}`}
                aria-label={t('筛选操作')}
                aria-haspopup="menu"
                aria-expanded={filterOpen}
                title={filter === 'all' ? t('筛选操作') : t('筛选：{0}', activeFilterLabel)}
                onClick={() => setFilterOpen((value) => !value)}
              >
                <Filter size={16} />
              </button>
              {filterOpen && (
                <div className="activity-filter-menu" role="menu" aria-label={t('筛选操作')}>
                  <button
                    role="menuitemradio"
                    aria-checked={filter === 'all'}
                    className={filter === 'all' ? 'active' : ''}
                    onClick={() => {
                      setFilter('all')
                      setFilterOpen(false)
                    }}
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
                          onClick={() => {
                            setFilter(value)
                            setFilterOpen(false)
                          }}
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
                            onClick={() => {
                              setFilter(value)
                              setFilterOpen(false)
                            }}
                          >
                            <span>{entry.label}</span>
                            {filter === value && <Check size={14} />}
                          </button>
                        )
                      })}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
          <button
            className="icon-button activity-close"
            aria-label={t('关闭状态中心')}
            onClick={() => {
              setSelectedId(null)
              activity.setOpen(false)
            }}
          >
            <X size={17} />
          </button>
        </div>
      </header>
      {selected ? (
        <ActivityDetails item={selected} />
      ) : (
        <>
          <ol className="activity-list">
            {filtered.length ? (
              filtered.map((item) => (
                <li key={item.id} className={`activity-item ${item.status}`}>
                  <span className="activity-icon">
                    <StatusIcon item={item} />
                  </span>
                  <span className="activity-main">
                    <span className="activity-label">{item.label}</span>
                    <small>
                      {item.status === 'running'
                        ? t('进行中')
                        : item.status === 'error'
                          ? t('失败')
                          : t('已完成')}
                      {item.status !== 'running' &&
                        t(' · 结束 {0}', time(item.finishedAt || item.startedAt))}
                      {item.status === 'running' && item.indeterminate && t(' · 处理中')}
                      {item.status === 'running' &&
                        item.progress !== undefined &&
                        !item.indeterminate &&
                        ` · ${Math.round(item.progress * 100)}%`}
                      {item.status === 'running' &&
                        item.total !== undefined &&
                        (item.unit === 'bytes'
                          ? t(' · {0} / {1}', size(item.done ?? 0), size(item.total))
                          : t(' · {0}/{1} 文件', item.done ?? 0, item.total))}
                      {item.status === 'running' && eta(item) && t(' · 预计 {0}', eta(item))}
                    </small>
                    {item.status === 'running' && (
                      <span
                        className={`activity-progress ${item.progress !== undefined ? 'determinate' : ''}`}
                        aria-label={
                          item.progress !== undefined
                            ? t('进度 {0}%', Math.round(item.progress * 100))
                            : t('进行中')
                        }
                      >
                        <i
                          style={
                            item.progress !== undefined
                              ? { width: `${Math.round(item.progress * 100)}%` }
                              : undefined
                          }
                        />
                      </span>
                    )}
                    {item.status === 'running' && item.step && (
                      <small className="activity-step">{item.step}</small>
                    )}
                  </span>
                  <button
                    className="activity-view"
                    onClick={() => setSelectedId(item.id)}
                    aria-label={t('查看详情：{0}', item.label)}
                  >
                    {t('查看详情')}
                    <ChevronRight size={13} />
                  </button>
                </li>
              ))
            ) : (
              <li className="activity-empty">
                {activity.items.length ? <Filter size={20} /> : <Clock3 size={20} />}
                <span>{activity.items.length ? t('没有符合条件的操作') : t('暂无操作')}</span>
                {activity.items.length > 0 && filter !== 'all' && (
                  <button type="button" onClick={() => setFilter('all')}>
                    {t('显示全部')}
                  </button>
                )}
              </li>
            )}
          </ol>
          <footer>
            <span>
              {filter === 'all'
                ? `${activity.items.length} ${t('条记录')}`
                : `${filtered.length} / ${activity.items.length} ${t('条记录')}`}
            </span>
            {activity.items.some((item) => item.status !== 'running') && (
              <button onClick={activity.clear}>{t('清除历史')}</button>
            )}
          </footer>
        </>
      )}
    </aside>
  )
}

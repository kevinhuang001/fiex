import { showLogs } from '@/lib/logCenter'
import { ActivityDetails, StatusIcon } from './activity/ActivityDetails'
import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useRef, useState } from 'react'
import { ArrowLeft, ChevronRight, Clock3, Filter, ScrollText, X } from 'lucide-react'
import { useActivity } from '@/hooks/useActivity'
import { usePluginCatalog } from '@/lib/plugins'
import { activityEta as eta } from '@/lib/activity'
import { size } from '@/lib/files'
import { ActivityFilterMenu } from './activity/ActivityFilterMenu'
import {
  getActivityFilterLabel,
  getActivityPluginFilters,
  matchesActivityFilter,
  type ActivityFilter,
} from './activity/filters'

const time = (value: number) =>
  new Date(value).toLocaleTimeString(getLocale(), { hour: '2-digit', minute: '2-digit' })

export function ActivityCenter() {
  const activity = useActivity()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [filter, setFilter] = useState<ActivityFilter>('all')
  const [filterOpen, setFilterOpen] = useState(false)
  const filterRef = useRef<HTMLDivElement>(null)
  const panel = useRef<HTMLElement>(null)
  const plugins = usePluginCatalog()
  const pluginFilters = useMemo(() => {
    return getActivityPluginFilters(activity.items, plugins)
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
  const filtered = activity.items.filter((item) => matchesActivityFilter(item, filter))
  const running = filtered.filter((item) => item.status === 'running').length
  const activeFilterLabel = getActivityFilterLabel(filter, pluginFilters)
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
          <button
            className="icon-button"
            title={t('运行日志')}
            aria-label={t('运行日志')}
            onClick={() => {
              activity.setOpen(false)
              showLogs()
            }}
          >
            <ScrollText size={16} />
          </button>
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
                <ActivityFilterMenu
                  filter={filter}
                  pluginFilters={pluginFilters}
                  onSelect={(value) => {
                    setFilter(value)
                    setFilterOpen(false)
                  }}
                />
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

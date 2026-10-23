import { useEffect, useRef, useState } from 'react'
import { ArrowLeft, Check, ChevronRight, CircleAlert, Clock3, LoaderCircle, X } from 'lucide-react'
import { cancelActivity, isCancellable, useActivity, type ActivityItem } from '@/hooks/useActivity'
import { activityEta as eta } from '@/lib/activity'

const time = (value: number) =>
  new Date(value).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })

function StatusIcon({ item }: { item: ActivityItem }) {
  return item.status === 'running' ? (
    <LoaderCircle size={17} className="spin" />
  ) : item.status === 'error' ? (
    <CircleAlert size={17} />
  ) : (
    <Check size={17} />
  )
}

function ActivityDetails({ item }: { item: ActivityItem }) {
  const state = item.status === 'running' ? '进行中' : item.status === 'error' ? '失败' : '已完成'
  const percent = item.progress === undefined ? null : Math.round(item.progress * 100)
  const etaText = eta(item)
  return (
    <div className="activity-details">
      <div className={`activity-detail-hero ${item.status}`}>
        <span className="activity-detail-icon">
          <StatusIcon item={item} />
        </span>
        <strong>{item.label}</strong>
        <span>{state}</span>
      </div>
      <dl className="activity-detail-fields">
        <div>
          <dt>开始时间</dt>
          <dd>{new Date(item.startedAt).toLocaleString('zh-CN')}</dd>
        </div>
        {percent !== null && (
          <div>
            <dt>进度</dt>
            <dd>
              {percent}%{item.total ? ` · ${item.done ?? 0}/${item.total} 文件` : ''}
            </dd>
          </div>
        )}
        {etaText && (
          <div>
            <dt>预计剩余</dt>
            <dd>{etaText}</dd>
          </div>
        )}
        {item.step && (
          <div>
            <dt>当前</dt>
            <dd className="activity-detail-step">{item.step}</dd>
          </div>
        )}
        {item.finishedAt && (
          <div>
            <dt>结束时间</dt>
            <dd>{new Date(item.finishedAt).toLocaleString('zh-CN')}</dd>
          </div>
        )}
        {item.finishedAt && (
          <div>
            <dt>耗时</dt>
            <dd>{((item.finishedAt - item.startedAt) / 1000).toFixed(1)} 秒</dd>
          </div>
        )}
      </dl>
      {item.status === 'running' && (
        <div className="activity-detail-progress">
          <div className={`activity-progress ${percent !== null ? 'determinate' : ''}`}>
            <i style={percent !== null ? { width: `${percent}%` } : undefined} />
          </div>
          <span>{percent !== null ? `${percent}%` : '处理中'}</span>
        </div>
      )}
      {item.detail && <div className={`activity-detail-message ${item.status}`}>{item.detail}</div>}
      {item.status === 'running' && isCancellable(item.id) && (
        <button className="activity-detail-cancel" onClick={() => cancelActivity(item.id)}>
          取消操作
        </button>
      )}
    </div>
  )
}

export function ActivityCenter() {
  const activity = useActivity()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const panel = useRef<HTMLElement>(null)
  useEffect(() => {
    if (!activity.open) {
      setSelectedId(null)
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
  if (!activity.open) return null
  const selected = activity.items.find((item) => item.id === selectedId)
  const running = activity.items.filter((item) => item.status === 'running').length
  return (
    <aside ref={panel} className="activity-center" aria-label="状态中心">
      <header>
        {selected && (
          <button
            className="icon-button"
            aria-label="返回操作列表"
            onClick={() => setSelectedId(null)}
          >
            <ArrowLeft size={17} />
          </button>
        )}
        <strong>{selected ? '操作详情' : '状态中心'}</strong>
        {!selected && running > 0 && <span className="activity-running">{running} 进行中</span>}
        <button
          className="icon-button activity-close"
          aria-label="关闭状态中心"
          onClick={() => {
            setSelectedId(null)
            activity.setOpen(false)
          }}
        >
          <X size={17} />
        </button>
      </header>
      {selected ? (
        <ActivityDetails item={selected} />
      ) : (
        <>
          <ol className="activity-list">
            {activity.items.length ? (
              activity.items.map((item) => (
                <li key={item.id} className={`activity-item ${item.status}`}>
                  <span className="activity-icon">
                    <StatusIcon item={item} />
                  </span>
                  <span className="activity-main">
                    <span className="activity-label">{item.label}</span>
                    <small>
                      {item.status === 'running'
                        ? '进行中'
                        : item.status === 'error'
                          ? '失败'
                          : '已完成'}
                      {item.status !== 'running' &&
                        ` · 结束 ${time(item.finishedAt || item.startedAt)}`}
                      {item.status === 'running' &&
                        item.progress !== undefined &&
                        ` · ${Math.round(item.progress * 100)}%`}
                      {item.status === 'running' &&
                        item.total !== undefined &&
                        ` · ${item.done ?? 0}/${item.total} 文件`}
                      {item.status === 'running' && eta(item) && ` · 预计 ${eta(item)}`}
                    </small>
                    {item.status === 'running' && (
                      <span
                        className={`activity-progress ${item.progress !== undefined ? 'determinate' : ''}`}
                        aria-label={
                          item.progress !== undefined
                            ? `进度 ${Math.round(item.progress * 100)}%`
                            : '进行中'
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
                    aria-label={`查看详情：${item.label}`}
                  >
                    查看详情 <ChevronRight size={13} />
                  </button>
                </li>
              ))
            ) : (
              <li className="activity-empty">
                <Clock3 size={20} />
                <span>暂无操作</span>
              </li>
            )}
          </ol>
          <footer>
            <span>{activity.items.length} 条记录</span>
            {activity.items.some((item) => item.status !== 'running') && (
              <button onClick={activity.clear}>清除历史</button>
            )}
          </footer>
        </>
      )}
    </aside>
  )
}

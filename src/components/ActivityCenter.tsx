import { CheckCircle2, CircleAlert, History, LoaderCircle, X } from 'lucide-react'
import { cancelActivity, isCancellable, useActivity } from '@/hooks/useActivity'

const time = (value: number) =>
  new Date(value).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })

/// Live operations plus the history of everything Fiex has run, opened from
/// the status bar or automatically when a transfer starts.
export function ActivityCenter() {
  const activity = useActivity()
  if (!activity.open) return null
  const running = activity.items.filter((item) => item.status === 'running').length
  return (
    <aside className="activity-center" aria-label="状态中心">
      <header>
        <History size={15} />
        <span>状态中心</span>
        {running > 0 && <span className="activity-running">{running} 个进行中</span>}
        <button
          className="icon-button"
          aria-label="关闭状态中心"
          onClick={() => activity.setOpen(false)}
        >
          <X size={15} />
        </button>
      </header>
      <ol className="activity-list">
        {activity.items.length ? (
          activity.items.map((item) => (
            <li key={item.id} className={`activity-item ${item.status}`}>
              <span className="activity-icon">
                {item.status === 'running' ? (
                  <LoaderCircle size={14} className="spin" />
                ) : item.status === 'error' ? (
                  <CircleAlert size={14} />
                ) : (
                  <CheckCircle2 size={14} />
                )}
              </span>
              <span className="activity-main">
                <span className="activity-label">{item.label}</span>
                {item.status === 'running' && (
                  <span className="activity-progress" aria-label="进行中">
                    <i />
                  </span>
                )}
                {item.detail && <small title={item.detail}>{item.detail}</small>}
              </span>
              {item.status === 'running' && isCancellable(item.id) && (
                <button className="activity-cancel" onClick={() => cancelActivity(item.id)}>
                  取消
                </button>
              )}
              <span className="activity-time">{time(item.finishedAt || item.startedAt)}</span>
            </li>
          ))
        ) : (
          <li className="activity-empty">还没有操作记录</li>
        )}
      </ol>
      <footer>
        <span>{activity.items.length} 条记录</span>
        <button onClick={activity.clear}>清除历史</button>
      </footer>
    </aside>
  )
}

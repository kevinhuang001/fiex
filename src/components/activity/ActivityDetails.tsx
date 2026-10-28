import { t, getLocale } from '@/i18n'
import { Check, LoaderCircle, X } from 'lucide-react'
import { cancelActivity, isCancellable, type ActivityItem } from '@/hooks/useActivity'
import { activityEta as eta } from '@/lib/activity'
import { size } from '@/lib/files'
export function StatusIcon({ item }: { item: ActivityItem }) {
  return item.status === 'running' ? (
    <LoaderCircle size={17} className="spin" />
  ) : item.status === 'error' ? (
    <X size={17} />
  ) : (
    <Check size={17} />
  )
}

export function ActivityDetails({ item }: { item: ActivityItem }) {
  const state =
    item.status === 'running' ? t('进行中') : item.status === 'error' ? t('失败') : t('已完成')
  const percent =
    item.indeterminate || item.progress === undefined ? null : Math.round(item.progress * 100)
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
          <dt>{t('开始时间')}</dt>
          <dd>{new Date(item.startedAt).toLocaleString(getLocale())}</dd>
        </div>
        {percent !== null && (
          <div>
            <dt>{t('进度')}</dt>
            <dd>
              {percent}%
              {item.total
                ? item.unit === 'bytes'
                  ? t(' · {0} / {1}', size(item.done ?? 0), size(item.total))
                  : t(' · {0}/{1} 文件', item.done ?? 0, item.total)
                : ''}
            </dd>
          </div>
        )}
        {etaText && (
          <div>
            <dt>{t('预计剩余')}</dt>
            <dd>{etaText}</dd>
          </div>
        )}
        {item.step && (
          <div>
            <dt>{t('当前')}</dt>
            <dd className="activity-detail-step">{item.step}</dd>
          </div>
        )}
        {item.finishedAt && (
          <div>
            <dt>{t('结束时间')}</dt>
            <dd>{new Date(item.finishedAt).toLocaleString(getLocale())}</dd>
          </div>
        )}
        {item.finishedAt && (
          <div>
            <dt>{t('耗时')}</dt>
            <dd>
              {((item.finishedAt - item.startedAt) / 1000).toFixed(1)} {t('秒')}
            </dd>
          </div>
        )}
      </dl>
      {item.status === 'running' && (
        <div className="activity-detail-progress">
          <div
            className={`activity-progress ${percent !== null && !item.indeterminate ? 'determinate' : ''}`}
            aria-label={item.indeterminate ? t('处理中') : undefined}
          >
            <i
              style={percent !== null && !item.indeterminate ? { width: `${percent}%` } : undefined}
            />
          </div>
          <span>{percent !== null && !item.indeterminate ? `${percent}%` : t('处理中')}</span>
        </div>
      )}
      {item.detail && <div className={`activity-detail-message ${item.status}`}>{item.detail}</div>}
      {item.status === 'running' && isCancellable(item.id) && (
        <button className="activity-detail-cancel" onClick={() => cancelActivity(item.id)}>
          {t('取消操作')}
        </button>
      )}
    </div>
  )
}

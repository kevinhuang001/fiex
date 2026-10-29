import { t, getLocale } from '@/i18n'
import { useEffect, useState } from 'react'
import { call } from '@/lib/api'
import { Button } from '@/components/ui/button'

export interface LogRecord {
  at: number
  level: 'trace' | 'debug' | 'info' | 'warn' | 'error'
  source: string
  pluginId?: string
  operation?: string
  message: string
  fields?: Record<string, unknown> | null
}

const levels = ['', 'trace', 'debug', 'info', 'warn', 'error'] as const

/** Live, paged-by-cap runtime diagnostics. The Host also records lifecycle and
 * RPC events, so this view remains useful even if a plugin logs nothing. */
export function PluginLogsSection({ pluginId }: { pluginId: string }) {
  const [records, setRecords] = useState<LogRecord[]>([])
  const [level, setLevel] = useState<string>('')
  const [query, setQuery] = useState('')
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState('')

  useEffect(() => {
    let alive = true
    const update = async () => {
      try {
        const result = await call<LogRecord[]>('plugin_logs', {
          pluginId,
          level: level || null,
          limit: 500,
        })
        if (alive) {
          setRecords((previous) =>
            JSON.stringify(previous) === JSON.stringify(result) ? previous : result,
          )
          setLoadError('')
        }
      } catch (error) {
        if (alive) setLoadError(String(error))
      } finally {
        if (alive) setLoading(false)
      }
    }
    setLoading(true)
    void update()
    const timer = window.setInterval(() => {
      if (!document.hidden) void update()
    }, 1400)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [pluginId, level])

  const filtered = records.filter((record) =>
    [record.message, record.source, record.operation || ''].some((value) =>
      value.toLowerCase().includes(query.toLowerCase()),
    ),
  )
  const format = (record: LogRecord) =>
    `${new Date(record.at).toISOString()} [${record.level.toUpperCase()}] ${record.source}${record.operation ? ' / ' + record.operation : ''}: ${record.message}${record.fields && Object.keys(record.fields).length ? ' ' + JSON.stringify(record.fields) : ''}`
  const clear = async () => {
    try {
      await call('plugin_logs_clear', { pluginId })
      setRecords([])
      setLoadError('')
    } catch (error) {
      setLoadError(String(error))
    }
  }
  return (
    <section className="plugin-permission-section plugin-runtime-logs">
      <div className="plugin-section-title">
        <strong>{t('运行日志')}</strong>
        <small>{loading ? t('读取中…') : t('{0} 条', filtered.length)}</small>
      </div>
      <div
        className="plugin-log-controls"
        style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 12 }}
      >
        <select value={level} onChange={(e) => setLevel(e.target.value)} aria-label={t('日志等级')}>
          {levels.map((value) => (
            <option key={value} value={value}>
              {value ? value.toUpperCase() : t('全部')}
            </option>
          ))}
        </select>
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t('搜索日志')}
          aria-label={t('搜索日志')}
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => void navigator.clipboard.writeText(filtered.map(format).join('\n'))}
        >
          {t('复制')}
        </Button>
        <Button type="button" variant="outline" size="sm" onClick={() => void clear()}>
          {t('清空记录')}
        </Button>
      </div>
      {loadError && <p role="alert">{loadError}</p>}
      <ul
        className="plugin-error-list"
        aria-live="polite"
        style={{ maxHeight: '55vh', overflowY: 'auto' }}
      >
        {filtered.map((record, index) => (
          <li key={`${record.at}-${index}`}>
            <span className={`plugin-error-source ${record.level}`}>
              {record.level.toUpperCase()}
            </span>
            <div>
              <p>{record.message}</p>
              <small>
                {record.source}
                {record.operation && ` · ${record.operation}`}
              </small>
              {record.fields && Object.keys(record.fields).length > 0 && (
                <pre>{JSON.stringify(record.fields, null, 2)}</pre>
              )}
              <time>{new Date(record.at).toLocaleString(getLocale())}</time>
            </div>
          </li>
        ))}
      </ul>
      {!loading && !filtered.length && !loadError && <p>{t('没有记录')}</p>}
    </section>
  )
}

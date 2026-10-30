import { redactDiagnostic } from '@/lib/redactDiagnostic'
import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useRef, useState } from 'react'
import { call } from '@/lib/api'
import {
  Copy,
  Download,
  LoaderCircle,
  Pause,
  Play,
  Search,
  Terminal,
  Trash2,
  Circle,
} from 'lucide-react'

interface LogRecord {
  id: number
  at: number
  source: string
  level: 'trace' | 'debug' | 'info' | 'warn' | 'error'
  message: string
  detail?: string
}

const levels = ['all', 'trace', 'debug', 'info', 'warn', 'error'] as const
type LevelFilter = (typeof levels)[number]
const severity: Record<Exclude<LevelFilter, 'all'>, number> = {
  trace: 0,
  debug: 1,
  info: 2,
  warn: 3,
  error: 4,
}

/** The plugin log view never reads other plugins' streams. All querying is
 * scoped by the host-managed plugin identity and entries are rendered as text. */
export function PluginLogViewer({ pluginId }: { pluginId: string }) {
  const [entries, setEntries] = useState<LogRecord[]>([])
  const [level, setLevel] = useState<LevelFilter>('warn')
  const [query, setQuery] = useState('')
  const [paused, setPaused] = useState(false)
  const [loadError, setLoadError] = useState('')
  const [loading, setLoading] = useState(true)
  const generation = useRef(0)
  const pausedRef = useRef(paused)
  pausedRef.current = paused
  useEffect(() => {
    let active = true
    let cursor = 0
    let loadingPage = false
    const load = async () => {
      if (loadingPage) return
      loadingPage = true
      const token = generation.current
      try {
        const page = await call<{ entries: LogRecord[]; cursor: number }>('plugin_logs', {
          pluginId,
          after: cursor,
        })
        if (active && token === generation.current) {
          setLoadError('')
          cursor = page.cursor
          setEntries((old) =>
            [...page.entries, ...old.filter((r) => !page.entries.some((v) => v.id === r.id))].slice(
              0,
              500,
            ),
          )
        }
      } catch {
        if (active) setLoadError(t('日志暂时不可用'))
      } finally {
        loadingPage = false
        if (active) setLoading(false)
      }
    }
    setEntries([])
    setLoading(true)
    void load()
    const refresh = window.setInterval(() => {
      if (!pausedRef.current) void load()
    }, 2000)
    return () => {
      active = false
      window.clearInterval(refresh)
    }
  }, [pluginId])
  const visible = useMemo(
    () =>
      entries.filter((entry) => {
        if (level !== 'all' && severity[entry.level] < severity[level]) return false
        const term = query.trim().toLowerCase()
        return (
          !term ||
          [entry.source, entry.message, entry.detail ?? ''].some((part) =>
            part.toLowerCase().includes(term),
          )
        )
      }),
    [entries, level, query],
  )
  const formatted = redactDiagnostic(
    visible
      .map(
        (entry) =>
          `${new Date(entry.at).toISOString()} [${entry.level.toUpperCase()}] [${entry.source}] ${entry.message}${entry.detail ? '\n' + entry.detail : ''}`,
      )
      .join('\n'),
    256 * 1024,
  )
  const clear = async () => {
    try {
      generation.current += 1
      await call('plugin_logs_clear', { pluginId })
      setEntries([])
    } catch {
      // The plugin continues normally if diagnostic history cannot be cleared.
    }
  }
  const exportLogs = () => {
    const file = new Blob([formatted], { type: 'text/plain;charset=utf-8' })
    const url = URL.createObjectURL(file)
    const link = document.createElement('a')
    link.href = url
    link.download = 'fiex-plugin-logs.txt'
    link.click()
    URL.revokeObjectURL(url)
  }
  return (
    <section className="runtime-logs">
      <div className="runtime-log-toolbar">
        <label className="runtime-log-search">
          <Search size={15} />
          <input
            aria-label={t('搜索日志')}
            placeholder={t('搜索日志')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <select
          aria-label={t('日志级别')}
          value={level}
          onChange={(e) => setLevel(e.target.value as LevelFilter)}
        >
          {levels.map((value) => (
            <option key={value} value={value}>
              {value === 'all' ? t('所有级别') : value.toUpperCase()}
            </option>
          ))}
        </select>
        <button
          className={`icon-button ${paused ? 'active' : ''}`}
          aria-label={paused ? t('继续') : t('暂停')}
          title={paused ? t('继续') : t('暂停')}
          onClick={() => setPaused((v) => !v)}
        >
          {paused ? <Play size={15} /> : <Pause size={15} />}
        </button>
        <button
          className="icon-button"
          aria-label={t('复制日志')}
          title={t('复制日志')}
          onClick={() => void navigator.clipboard.writeText(formatted).catch(() => {})}
        >
          <Copy size={15} />
        </button>
        <button
          className="icon-button"
          aria-label={t('导出日志')}
          title={t('导出日志')}
          onClick={exportLogs}
        >
          <Download size={15} />
        </button>
        <button
          className="icon-button"
          aria-label={t('清空记录')}
          title={t('清空记录')}
          onClick={() => void clear()}
        >
          <Trash2 size={15} />
        </button>
      </div>
      <div className="runtime-log-summary">
        <span className={paused ? 'paused' : 'live'}>
          <Circle size={7} fill="currentColor" />
          {paused ? t('已暂停') : t('实时更新')}
        </span>
        <span>{t('{0} 条', visible.length)}</span>
        <span className="log-count-error">
          {entries.filter((v) => v.level === 'error').length} ERROR
        </span>
        <span className="log-count-warn">
          {entries.filter((v) => v.level === 'warn').length} WARN
        </span>
      </div>
      {loadError && (
        <p className="settings-error" role="status">
          {loadError}
        </p>
      )}
      <div className="runtime-log-list" role="log" aria-label={t('运行日志')} aria-live="off">
        {loading ? (
          <div className="runtime-log-empty">
            <LoaderCircle className="spin" size={24} />
            <p>{t('读取中…')}</p>
          </div>
        ) : visible.length === 0 ? (
          <div className="runtime-log-empty">
            <Terminal size={30} />
            <strong>{query || level !== 'all' ? t('没有匹配的日志') : t('暂无日志')}</strong>
            <p>{t('新的执行记录会自动显示在这里。')}</p>
          </div>
        ) : (
          visible.map((entry) => (
            <details className={`runtime-log-row level-${entry.level}`} key={entry.id}>
              <summary>
                <time>{new Date(entry.at).toLocaleTimeString(getLocale(), { hour12: false })}</time>
                <span className="runtime-log-level">{entry.level.toUpperCase()}</span>
                <span className="runtime-log-message">{entry.message}</span>
                <span className="runtime-log-component" title={entry.source}>
                  {entry.source}
                </span>
              </summary>
              <div className="runtime-log-detail">
                <span>
                  {new Date(entry.at).toLocaleString(getLocale())} · #{entry.id}
                </span>
                <pre>{entry.detail || entry.message}</pre>
              </div>
            </details>
          ))
        )}
      </div>
    </section>
  )
}

import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useRef, useState } from 'react'
import { call } from '@/lib/api'
import { Button } from '@/components/ui/button'

interface LogRecord {
  at: number
  source: string
  level: 'trace' | 'debug' | 'info' | 'warn' | 'error'
  message: string
  detail?: string
}

const levels = ['all', 'trace', 'debug', 'info', 'warn', 'error'] as const
type LevelFilter = (typeof levels)[number]

/** The plugin log view never reads other plugins' streams. All querying is
 * scoped by the host-managed plugin identity and entries are rendered as text. */
export function PluginLogViewer({ pluginId }: { pluginId: string }) {
  const [entries, setEntries] = useState<LogRecord[]>([])
  const [level, setLevel] = useState<LevelFilter>('all')
  const [query, setQuery] = useState('')
  const [paused, setPaused] = useState(false)
  const [loading, setLoading] = useState(true)
  const pausedRef = useRef(paused)
  pausedRef.current = paused
  useEffect(() => {
    let active = true
    const load = async () => {
      try {
        const values = await call<LogRecord[]>('plugin_logs', { pluginId })
        if (active) setEntries(values)
      } catch {
        // Diagnostics may be unavailable while Core shuts down.
      } finally {
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
        if (level !== 'all' && entry.level !== level) return false
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
  const formatted = visible
    .map(
      (entry) =>
        `${new Date(entry.at).toISOString()} [${entry.level.toUpperCase()}] [${entry.source}] ${entry.message}${entry.detail ? '\n' + entry.detail : ''}`,
    )
    .join('\n')
  const clear = async () => {
    try {
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
    <section className="plugin-permission-section">
      <div className="plugin-section-title">
        <strong>{t('插件日志')}</strong>
        <small>{loading ? t('读取中…') : t('{0} 条', visible.length)}</small>
      </div>
      <div className="plugin-log-toolbar">
        <select
          aria-label={t('日志级别')}
          value={level}
          onChange={(event) => setLevel(event.target.value as LevelFilter)}
        >
          {levels.map((value) => (
            <option key={value} value={value}>
              {value.toUpperCase()}
            </option>
          ))}
        </select>
        <input
          aria-label={t('搜索日志')}
          value={query}
          placeholder={t('搜索日志')}
          onChange={(event) => setQuery(event.target.value)}
        />
        <Button type="button" size="sm" variant="outline" onClick={() => setPaused((x) => !x)}>
          {paused ? t('继续') : t('暂停')}
        </Button>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => void navigator.clipboard.writeText(formatted).catch(() => {})}
        >
          {t('复制')}
        </Button>
        <Button type="button" size="sm" variant="outline" onClick={exportLogs}>
          {t('导出')}
        </Button>
        <Button type="button" size="sm" variant="outline" onClick={() => void clear()}>
          {t('清空记录')}
        </Button>
      </div>
      <ul className="plugin-error-list">
        {visible.map((entry, index) => (
          <li key={`${entry.at}-${index}`}>
            <span className={`plugin-error-source ${entry.level}`}>
              {entry.level.toUpperCase()}
            </span>
            <div>
              <small>{entry.source}</small>
              <p>{entry.message}</p>
              {entry.detail && <pre>{entry.detail}</pre>}
              <time>{new Date(entry.at).toLocaleString(getLocale())}</time>
            </div>
          </li>
        ))}
      </ul>
    </section>
  )
}

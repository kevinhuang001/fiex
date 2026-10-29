import { t, getLocale } from '@/i18n'
import { useEffect, useState } from 'react'
import { AlertTriangle } from 'lucide-react'
import { call } from '@/lib/api'
import { Button } from '@/components/ui/button'

export type PluginError = {
  at: number
  source: string
  level: string
  message: string
  detail?: string
}

/** Recent failures Core observed or the plugin reported. Diagnostics only: it
 * never blocks the plugin, and "clear" only empties this log. */
export function PluginErrorsSection({ pluginId }: { pluginId: string }) {
  const [errors, setErrors] = useState<PluginError[]>([])
  const [loading, setLoading] = useState(false)
  const [level, setLevel] = useState('all')
  useEffect(() => {
    let alive = true
    const load = async () => {
      try {
        const items = await call<PluginError[]>('plugin_errors', { pluginId })
        if (alive) setErrors(items || [])
      } catch {
        // A temporary diagnostics failure must not disrupt plugin settings.
      } finally {
        if (alive) setLoading(false)
      }
    }
    setLoading(true)
    void load()
    const interval = window.setInterval(() => void load(), 2_000)
    return () => {
      alive = false
      window.clearInterval(interval)
    }
  }, [pluginId])

  const clear = () => {
    void call('plugin_error_clear', { pluginId })
      .then(() => setErrors([]))
      .catch(() => {})
  }
  return (
    <section className="plugin-permission-section">
      <div className="plugin-section-title">
        <strong>
          <AlertTriangle size={15} /> {t('插件日志')}
        </strong>
        <small>
          {loading ? t('读取中…') : errors.length ? t('{0} 条', errors.length) : t('没有记录')}
        </small>
      </div>
      <select aria-label="日志级别" value={level} onChange={(event) => setLevel(event.target.value)}>
        <option value="all">All</option>
        <option value="trace">Trace</option>
        <option value="debug">Debug</option>
        <option value="info">Info</option>
        <option value="warn">Warn</option>
        <option value="error">Error</option>
      </select>
      {errors.length ? (
        <>
          <ul className="plugin-error-list">
            {errors.filter((entry) => level === 'all' || (entry.level || 'error') === level).map((entry, index) => (
              <li key={`${entry.at}-${index}`}>
                <span className={`plugin-error-source ${entry.level || "error"}`}>{(entry.level || "error").toUpperCase()}</span>
                <div>
                  <small>{entry.source}</small>
                  <p>{entry.message}</p>
                  {entry.detail && <pre>{entry.detail}</pre>}
                  <time>{new Date(entry.at).toLocaleString(getLocale())}</time>
                </div>
              </li>
            ))}
          </ul>
          <Button type="button" size="sm" variant="outline" onClick={clear}>
            {t('清空记录')}
          </Button>
        </>
      ) : null}
    </section>
  )
}

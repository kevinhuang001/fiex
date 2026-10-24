import { useEffect, useState } from 'react'
import { AlertTriangle } from 'lucide-react'
import { call } from '@/lib/api'
import { Button } from '@/components/ui/button'

export type PluginError = {
  at: number
  source: string
  message: string
  detail?: string
}

/** Recent failures Core observed or the plugin reported. Diagnostics only: it
 * never blocks the plugin, and "clear" only empties this log. */
export function PluginErrorsSection({ pluginId }: { pluginId: string }) {
  const [errors, setErrors] = useState<PluginError[]>([])
  const [loading, setLoading] = useState(false)
  useEffect(() => {
    let alive = true
    setLoading(true)
    void call<PluginError[]>('plugin_errors', { pluginId })
      .then((items) => {
        if (alive) setErrors(items || [])
      })
      .catch(() => {})
      .finally(() => {
        if (alive) setLoading(false)
      })
    return () => {
      alive = false
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
          <AlertTriangle size={15} /> 最近错误
        </strong>
        <small>{loading ? '读取中…' : errors.length ? `${errors.length} 条` : '没有记录'}</small>
      </div>
      {errors.length ? (
        <>
          <ul className="plugin-error-list">
            {errors.map((entry, index) => (
              <li key={`${entry.at}-${index}`}>
                <span className={`plugin-error-source ${entry.source}`}>{entry.source}</span>
                <div>
                  <p>{entry.message}</p>
                  {entry.detail && <pre>{entry.detail}</pre>}
                  <time>{new Date(entry.at).toLocaleString()}</time>
                </div>
              </li>
            ))}
          </ul>
          <Button type="button" size="sm" variant="outline" onClick={clear}>
            清空记录
          </Button>
        </>
      ) : (
        <p className="sidebar-hint">命令缺失、进程退出、面板或 RPC 失败都会记录在这里。</p>
      )}
    </section>
  )
}

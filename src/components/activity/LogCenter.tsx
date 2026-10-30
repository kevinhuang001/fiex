import { useState } from 'react'
import { Cpu, Puzzle, Search, ScrollText } from 'lucide-react'
import { t } from '@/i18n'
import { Modal } from '@/components/ui/Modal'
import { usePluginCatalog } from '@/lib/plugins'
import { closeLogs, showLogs, useLogSource } from '@/lib/logCenter'
import { PluginLogViewer } from '@/components/settings/plugins/PluginLogViewer'

export function LogCenter() {
  const source = useLogSource()
  const plugins = usePluginCatalog()
  const [query, setQuery] = useState('')
  const sources = [
    { id: 'core', name: t('应用核心'), core: true },
    ...plugins.map((p) => ({ id: p.id, name: p.name, core: false })),
  ]
  const selected = sources.find((item) => item.id === source)
  return (
    <Modal
      open={source !== null}
      onClose={closeLogs}
      title={t('运行日志')}
      className="log-center-modal"
    >
      {source !== null && (
        <div className="log-center-layout">
          <aside className="log-sources">
            <div className="log-source-search">
              <Search size={14} />
              <input
                aria-label={t('筛选日志来源')}
                placeholder={t('筛选日志来源')}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
            </div>
            <div className="log-source-label">{t('来源')}</div>
            {sources
              .filter((item) => item.name.toLowerCase().includes(query.toLowerCase()))
              .map((item) => (
                <button
                  key={item.id}
                  className={item.id === source ? 'active' : ''}
                  onClick={() => showLogs(item.id)}
                >
                  {item.core ? <Cpu size={16} /> : <Puzzle size={16} />}
                  <span>{item.name}</span>
                  {item.id === source && <i />}
                </button>
              ))}
          </aside>
          <main className="log-center-main">
            <div className="log-center-title">
              <div className="log-center-title-icon">
                <ScrollText size={21} />
              </div>
              <div>
                <h2>{selected?.name ?? source}</h2>
                <p>{t('执行记录与诊断信息')}</p>
              </div>
            </div>
            <PluginLogViewer key={source} pluginId={source} />
          </main>
        </div>
      )}
    </Modal>
  )
}

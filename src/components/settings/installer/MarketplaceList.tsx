import { Check, Download, LoaderCircle, Puzzle, Search } from 'lucide-react'
import type { Dispatch, SetStateAction } from 'react'
import type { MarketplacePlugin } from '@/lib/plugins'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import { pluginIcon } from './pluginIcon'

/** The catalog sheet: search, alerts and the installable plugin grid. */
export function MarketplaceList({
  open,
  installedIds,
  onClose,
  query,
  setQuery,
  error,
  message,
  loading,
  entries,
  busyId,
  onPrepare,
}: {
  open: boolean
  installedIds: string[]
  onClose: () => void
  query: string
  setQuery: Dispatch<SetStateAction<string>>
  error: string
  message: string
  loading: boolean
  entries: MarketplacePlugin[]
  busyId: string
  onPrepare: (entry: MarketplacePlugin) => void
}) {
  const filtered = entries.filter((entry) => {
    const term = query.trim().toLowerCase()
    return (
      !term ||
      entry.name.toLowerCase().includes(term) ||
      entry.id.toLowerCase().includes(term) ||
      entry.description.toLowerCase().includes(term)
    )
  })
  return (
    <Modal open={open} onClose={onClose} title="获取插件" className="plugin-store-modal">
      <div className="plugin-store">
        <div className="plugin-store-tools">
          <label className="plugin-store-search">
            <Search size={15} />
            <input
              aria-label="搜索插件"
              placeholder="搜索插件"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
        </div>
        {error && (
          <p role="alert" className="connection-error">
            {error}
          </p>
        )}
        {message && (
          <p role="status" className="plugin-store-message">
            {message}
          </p>
        )}
        {loading && !entries.length ? (
          <div className="plugin-store-empty">
            <LoaderCircle className="spin" size={20} /> 正在读取插件目录…
          </div>
        ) : (
          <div className="plugin-store-grid">
            {filtered.map((entry) => {
              const installed = installedIds.includes(entry.id)
              return (
                <article className="plugin-store-card" key={entry.id}>
                  <div className="plugin-store-card-head">
                    <div className={`plugin-store-icon ${installed ? 'installed' : ''}`}>
                      {pluginIcon(entry.icon)}
                    </div>
                    <div className="plugin-store-main">
                      <div className="plugin-store-title">
                        <strong>{entry.name}</strong>
                        <span className="plugin-pill">v{entry.version}</span>
                        {installed && (
                          <span className="plugin-pill success">
                            <Check size={11} /> 已安装
                          </span>
                        )}
                      </div>
                      <p>{entry.description || '没有提供说明。'}</p>
                    </div>
                  </div>
                  <Button
                    size="sm"
                    variant={installed ? 'outline' : 'default'}
                    disabled={installed || Boolean(busyId)}
                    onClick={() => void onPrepare(entry)}
                  >
                    {busyId === entry.id ? (
                      <LoaderCircle className="spin" size={14} />
                    ) : (
                      <Download size={14} />
                    )}
                    {installed ? '已安装' : busyId === entry.id ? '获取中…' : '获取'}
                  </Button>
                </article>
              )
            })}
            {!filtered.length && !loading && (
              <div className="plugin-store-empty">
                <Puzzle size={22} />
                <span>{entries.length ? '没有匹配的插件' : '暂时没有可安装的插件'}</span>
              </div>
            )}
          </div>
        )}
      </div>
    </Modal>
  )
}

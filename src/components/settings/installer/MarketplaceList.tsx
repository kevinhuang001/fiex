import { Check, Download, LoaderCircle, Puzzle, RefreshCw, Search } from 'lucide-react'
import { useEffect, useState, type Dispatch, type SetStateAction } from 'react'
import {
  PLUGIN_CATEGORIES,
  pluginCategories,
  isNewerVersion,
  type MarketplacePlugin,
  type PluginInfo,
} from '@/lib/plugins'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import { pluginIcon } from './pluginIcon'
import { PluginCategoryFilter } from '../plugins/PluginCategoryFilter'

/** The catalog sheet: search, alerts and the installable plugin grid. */
export function MarketplaceList({
  open,
  installed,
  onClose,
  query,
  setQuery,
  error,
  message,
  loading,
  entries,
  busyId,
  onPrepare,
  initialCategory,
}: {
  open: boolean
  installed: PluginInfo[]
  onClose: () => void
  query: string
  setQuery: Dispatch<SetStateAction<string>>
  error: string
  message: string
  loading: boolean
  entries: MarketplacePlugin[]
  busyId: string
  onPrepare: (entry: MarketplacePlugin) => void
  initialCategory?: string
}) {
  const [category, setCategory] = useState(initialCategory || 'all')
  useEffect(() => {
    if (open) setCategory(initialCategory || 'all')
  }, [open, initialCategory])
  const categories = (() => {
    const counts = new Map<string, number>()
    for (const entry of entries) {
      for (const id of pluginCategories(entry.extensions, entry.commands)) {
        counts.set(id, (counts.get(id) || 0) + 1)
      }
    }
    return [...counts].map(([id, count]) => ({ id, count }))
  })()
  const versions = new Map(installed.map((plugin) => [plugin.id, plugin.version]))
  const filtered = entries.filter((entry) => {
    const term = query.trim().toLowerCase()
    const matchesSearch =
      !term ||
      entry.name.toLowerCase().includes(term) ||
      entry.id.toLowerCase().includes(term) ||
      entry.description.toLowerCase().includes(term)
    const matchesCategory =
      category === 'all' || pluginCategories(entry.extensions, entry.commands).includes(category)
    return matchesSearch && matchesCategory
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
        <PluginCategoryFilter categories={categories} value={category} onChange={setCategory} />
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
              const currentVersion = versions.get(entry.id)
              const isInstalled = currentVersion !== undefined
              const isUpdate = isInstalled && isNewerVersion(entry.version, currentVersion)
              return (
                <article className="plugin-store-card" key={entry.id}>
                  <div className="plugin-store-card-head">
                    <div className={`plugin-store-icon ${isInstalled ? 'installed' : ''}`}>
                      {pluginIcon(entry.icon)}
                    </div>
                    <div className="plugin-store-main">
                      <div className="plugin-store-title">
                        <strong>{entry.name}</strong>
                        <span className="plugin-pill">v{entry.version}</span>
                        {isInstalled && !isUpdate && (
                          <span className="plugin-pill success">
                            <Check size={11} /> 已安装
                          </span>
                        )}
                        {isUpdate && (
                          <span className="plugin-pill warn">可更新至 v{entry.version}</span>
                        )}
                      </div>
                      <p>{entry.description || '没有提供说明。'}</p>
                      <div className="plugin-card-categories">
                        {pluginCategories(entry.extensions, entry.commands)
                          .slice(0, 4)
                          .map((id) => (
                            <span className="plugin-category-pill" key={id}>
                              {PLUGIN_CATEGORIES.find((item) => item.id === id)?.label || id}
                            </span>
                          ))}
                      </div>
                    </div>
                  </div>
                  <Button
                    size="sm"
                    variant={isInstalled && !isUpdate ? 'outline' : 'default'}
                    disabled={(isInstalled && !isUpdate) || Boolean(busyId)}
                    onClick={() => void onPrepare(entry)}
                  >
                    {busyId === entry.id ? (
                      <LoaderCircle className="spin" size={14} />
                    ) : isUpdate ? (
                      <RefreshCw size={14} />
                    ) : (
                      <Download size={14} />
                    )}
                    {isUpdate
                      ? '更新'
                      : isInstalled
                        ? '已安装'
                        : busyId === entry.id
                          ? '获取中…'
                          : '获取'}
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

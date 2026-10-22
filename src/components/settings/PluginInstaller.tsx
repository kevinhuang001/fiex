import { useEffect, useState } from 'react'
import {
  Archive,
  Check,
  Cloud,
  Download,
  GitBranch,
  HardDrive,
  Hash,
  LoaderCircle,
  Puzzle,
  RefreshCw,
  Search,
  Shield,
  Store,
} from 'lucide-react'
import { call } from '@/lib/api'
import { pluginMarketplace, type MarketplacePlugin } from '@/lib/plugins'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'

export const permissionTitles: Record<string, string> = {
  'filesystem.read': '读取文件',
  'filesystem.write': '修改文件',
  network: '网络访问',
}

interface InstallPreview {
  ticket: string
  id: string
  name: string
  version: string
  author?: { name: string; url?: string }
  permissions: string[]
  requestedNetworkDomains: string[]
  repository: string
}

function pluginIcon(id: string) {
  if (id.includes('git')) return <GitBranch size={21} />
  if (id.includes('webdav')) return <Cloud size={21} />
  if (id.includes('smb')) return <HardDrive size={21} />
  if (id.includes('archive')) return <Archive size={21} />
  if (id.includes('checksum')) return <Hash size={21} />
  return <Puzzle size={21} />
}

/**
 * App-store style official plugin catalog. The repository and asset names are
 * a fixed release contract: `fiex-catalog.json` plus
 * `<slug>-<platform>.fiexplugin`. Nothing is started until the user reviews the
 * exact permissions and confirms installation.
 */
export function PluginInstaller({
  open,
  installedIds,
  onClose,
  onInstalled,
}: {
  open: boolean
  installedIds: string[]
  onClose: () => void
  onInstalled: () => void
}) {
  const [entries, setEntries] = useState<MarketplacePlugin[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [busyId, setBusyId] = useState('')
  const [preview, setPreview] = useState<InstallPreview | null>(null)
  const [installing, setInstalling] = useState(false)
  const [message, setMessage] = useState('')
  const [query, setQuery] = useState('')

  const load = () => {
    if (loading) return
    setLoading(true)
    setError('')
    setMessage('')
    void pluginMarketplace()
      .then(setEntries)
      .catch((cause) => setError(String(cause)))
      .finally(() => setLoading(false))
  }
  useEffect(() => {
    if (!open) return
    setPreview(null)
    setMessage('')
    if (!entries.length) load()
    // The catalog is intentionally fetched once per settings session.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  const prepare = async (entry: MarketplacePlugin) => {
    if (busyId) return
    setBusyId(entry.id)
    setError('')
    setMessage('')
    try {
      const data = await call<InstallPreview>('plugin_prepare_install', {
        repository: entry.repository,
        assetName: entry.asset,
      })
      setPreview(data)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusyId('')
    }
  }
  const discard = () => {
    const ticket = preview?.ticket
    setPreview(null)
    if (ticket) void call('plugin_cancel_install', { ticket }).catch(() => {})
  }
  const confirm = async () => {
    if (!preview || installing) return
    setInstalling(true)
    setError('')
    try {
      await call('plugin_confirm_install', { ticket: preview.ticket })
      const installed = preview
      setPreview(null)
      setMessage(`${installed.name} 已安装`)
      onInstalled()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setInstalling(false)
    }
  }

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
    <>
      <Modal open={open} onClose={onClose} title="获取插件" wide className="plugin-store-modal">
        <div className="plugin-store">
          <header className="plugin-store-hero">
            <div className="plugin-store-hero-icon">
              <Store size={24} />
            </div>
            <div>
              <strong>Fiex 官方插件</strong>
              <p>按需安装。所有插件都需要你审核权限后才会运行。</p>
            </div>
            <Button variant="outline" size="sm" disabled={loading} onClick={load}>
              {loading ? <LoaderCircle className="spin" size={14} /> : <RefreshCw size={14} />}
              刷新
            </Button>
          </header>
          <label className="plugin-store-search">
            <Search size={15} />
            <input
              aria-label="搜索插件"
              placeholder="搜索插件"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
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
                    <div className={`plugin-store-icon ${installed ? 'installed' : ''}`}>
                      {pluginIcon(entry.id)}
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
                      <div className="plugin-store-meta">
                        <span>{entry.author?.name || 'Fiex Contributors'}</span>
                        <span aria-hidden>·</span>
                        <span>{entry.permissions.length} 项权限</span>
                      </div>
                    </div>
                    <Button
                      size="sm"
                      variant={installed ? 'outline' : 'default'}
                      disabled={installed || Boolean(busyId)}
                      onClick={() => void prepare(entry)}
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
          <p className="plugin-store-footnote">
            安装包来自 GitHub Release。插件以独立进程运行，权限由 Fiex 宿主校验。
          </p>
        </div>
      </Modal>
      <Modal open={Boolean(preview)} title="确认插件安装及权限" onClose={discard}>
        <div className="plugin-install-review">
          <div className="plugin-install-review-heading">
            <Shield size={21} />
            <div>
              <strong>{preview?.name}</strong>
              <small>
                {preview?.id} · {preview?.version}
              </small>
            </div>
          </div>
          <div className="plugin-install-meta">
            <span>作者：{preview?.author?.name || '未知开发者'}</span>
            <span>来源：GitHub / {preview?.repository}</span>
          </div>
          <strong>请求的权限</strong>
          <div className="plugin-install-permissions">
            {preview?.permissions.length ? (
              preview.permissions.map((permission) => (
                <div key={permission}>
                  <Shield size={14} />
                  {permissionTitles[permission] || permission}
                </div>
              ))
            ) : (
              <span>无需宿主权限</span>
            )}
          </div>
          {!!preview?.requestedNetworkDomains.length && (
            <>
              <strong>插件申请的网络域名</strong>
              <div className="plugin-install-domains">
                {preview.requestedNetworkDomains.map((domain) => (
                  <span key={domain}>{domain}</span>
                ))}
              </div>
              <p className="sidebar-hint">安装确认会同时授权这些精确域名，之后可以增删。</p>
            </>
          )}
          <p className="sidebar-hint">
            所有插件（包括 Fiex 官方发布的插件）使用同样的授权流程。插件为 Native
            程序；在系统沙箱完善前，权限控制仅约束宿主代理的操作，不能阻止插件直接进行系统调用。
          </p>
          {error && (
            <p role="alert" className="connection-error">
              {error}
            </p>
          )}
          <footer className="plugin-consent-actions">
            <Button variant="outline" type="button" disabled={installing} onClick={discard}>
              取消
            </Button>
            <Button type="button" disabled={installing} onClick={() => void confirm()}>
              {installing ? '安装中…' : '同意权限并安装'}
            </Button>
          </footer>
        </div>
      </Modal>
    </>
  )
}

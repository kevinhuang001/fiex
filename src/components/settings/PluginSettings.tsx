import { useEffect, useState } from 'react'
import {
  Check,
  Database,
  Download,
  Globe2,
  KeyRound,
  LoaderCircle,
  Plus,
  Puzzle,
  Settings2,
  ShieldCheck,
  Trash2,
  X,
} from 'lucide-react'
import { call } from '@/lib/api'
import {
  refreshPluginCatalog,
  usePluginCatalog,
  type PluginField,
  type PluginInfo,
} from '@/lib/plugins'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import { PluginInstaller } from './PluginInstaller'

const permissionLabels: Record<string, string> = {
  'filesystem.read': '读取文件',
  'filesystem.write': '修改文件',
  network: '网络访问',
}
const permissionHints: Record<string, string> = {
  'filesystem.read': '读取你选择的目录和文件内容。',
  'filesystem.write': '创建、修改和删除用户文件。',
  network: '只允许连接下方白名单中的精确域名。',
}

function iconFor(_id: string) {
  return <Puzzle size={19} />
}

function PermissionList({
  plugin,
  busy,
  onToggle,
}: {
  plugin: PluginInfo
  busy: boolean
  onToggle: (permission: string, granted: boolean) => void
}) {
  const declared = plugin.declaredPermissions || []
  const granted = plugin.grantedPermissions || []
  return (
    <>
      {declared.map((permission) => (
        <label className="plugin-permission-card" key={permission}>
          <span className="plugin-permission-icon">
            <ShieldCheck size={15} />
          </span>
          <span className="plugin-permission-copy">
            <strong>{permissionLabels[permission] || permission}</strong>
            <small>{permissionHints[permission] || permission}</small>
          </span>
          <input
            type="checkbox"
            aria-label={`授权 ${permissionLabels[permission] || permission}`}
            checked={granted.includes(permission)}
            disabled={busy || !plugin.approved}
            onChange={(event) => onToggle(permission, event.target.checked)}
          />
        </label>
      ))}
      {!declared.length && <p className="sidebar-hint">此插件未声明受控宿主权限。</p>}
    </>
  )
}

export function PluginSettings() {
  const plugins = usePluginCatalog()
  const [storeOpen, setStoreOpen] = useState(false)
  const [permissionsFor, setPermissionsFor] = useState<PluginInfo | null>(null)
  const [settingsFor, setSettingsFor] = useState<PluginInfo | null>(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [draft, setDraft] = useState<Record<string, string | boolean>>({})
  const [permissionToApprove, setPermissionToApprove] = useState<string | null>(null)
  const [domainDraft, setDomainDraft] = useState('')
  const [domainToApprove, setDomainToApprove] = useState<string | null>(null)
  const [confirmInstall, setConfirmInstall] = useState(false)
  const [confirmRemove, setConfirmRemove] = useState<PluginInfo | null>(null)

  const fields = settingsFor?.contributes?.settings || []

  useEffect(() => {
    if (!settingsFor) return
    let alive = true
    setMessage('')
    void Promise.all(
      fields.map(async (field) => {
        if (field.type === 'secret') return [field.id, ''] as const
        const value = await call<string | boolean | null>('plugin_storage_get', {
          pluginId: settingsFor.id,
          scope: 'global',
          key: field.id,
        })
        return [
          field.id,
          value ?? field.defaultValue ?? (field.type === 'checkbox' ? false : ''),
        ] as const
      }),
    )
      .then((entries) => {
        if (alive) setDraft(Object.fromEntries(entries))
      })
      .catch((error) => {
        if (alive) setMessage(String(error))
      })
    return () => {
      alive = false
    }
    // Field definitions are manifest data; refetch when the plugin changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settingsFor?.id])

  const updateActivation = async (plugin: PluginInfo, enabled: boolean) => {
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_enabled', { pluginId: plugin.id, enabled })
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const updatePermission = async (plugin: PluginInfo, permission: string, granted: boolean) => {
    if (granted) {
      setPermissionToApprove(permission)
      return
    }
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_permission', { pluginId: plugin.id, permission, granted })
      setPermissionToApprove(null)
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const grantPermission = async (plugin: PluginInfo) => {
    if (!permissionToApprove) return
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_permission', {
        pluginId: plugin.id,
        permission: permissionToApprove,
        granted: true,
      })
      setPermissionToApprove(null)
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const updateDomain = async (plugin: PluginInfo, domain: string, granted: boolean) => {
    if (granted && domainToApprove !== domain) {
      setDomainToApprove(domain)
      return
    }
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_network_domain', { pluginId: plugin.id, domain, granted })
      setDomainToApprove(null)
      setDomainDraft('')
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const uninstall = async () => {
    if (!confirmRemove || busy) return
    setBusy(true)
    try {
      await call('plugin_uninstall', { pluginId: confirmRemove.id })
      setConfirmRemove(null)
      setPermissionsFor(null)
      refreshPluginCatalog()
      setMessage(`已卸载 ${confirmRemove.name}，插件配置与同步目录已保留。`)
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const approve = async (plugin: PluginInfo) => {
    setBusy(true)
    try {
      await call('plugin_approve_install', { pluginId: plugin.id })
      setConfirmInstall(false)
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const saveSettings = async () => {
    if (!settingsFor || busy) return
    setBusy(true)
    setMessage('')
    try {
      for (const field of fields) {
        const value = draft[field.id]
        if (field.type === 'secret') {
          if (typeof value === 'string' && value.length)
            await call('plugin_secret_set', {
              pluginId: settingsFor.id,
              scope: 'global',
              key: field.id,
              value,
            })
        } else {
          await call('plugin_storage_set', {
            pluginId: settingsFor.id,
            scope: 'global',
            key: field.id,
            value: field.type === 'number' ? Number(value) : value,
          })
        }
      }
      setMessage('插件设置已保存')
      setDraft((old) =>
        Object.fromEntries(
          Object.entries(old).map(([key, value]) =>
            fields.some((f) => f.id === key && f.type === 'secret') ? [key, ''] : [key, value],
          ),
        ),
      )
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      <div className="plugin-page">
        <header className="settings-section-heading">
          <Puzzle size={19} />
          <div>
            <h3>插件</h3>
            <p>可选功能以独立进程运行，安装和权限都由你决定。</p>
          </div>
        </header>

        <div className="plugin-toolbar">
          <div>
            <strong>{plugins.length ? `${plugins.length} 个已安装插件` : '还没有安装插件'}</strong>
            <span>官方与第三方插件使用同一套权限和进程隔离。</span>
          </div>
          <Button type="button" size="sm" onClick={() => setStoreOpen(true)}>
            <Download size={15} />
            获取插件
          </Button>
        </div>

        {plugins.length ? (
          <div className="plugin-list">
            {plugins.map((plugin) => {
              const declared = plugin.declaredPermissions || []
              const enabled = plugin.enabled && plugin.approved
              const hasSettings = (plugin.contributes?.settings || []).length > 0
              return (
                <article className="plugin-row" key={plugin.id}>
                  <div className={`plugin-row-icon ${enabled ? 'enabled' : ''}`}>
                    {iconFor(plugin.id)}
                  </div>
                  <div className="plugin-row-main">
                    <div className="plugin-row-title">
                      <strong>{plugin.name}</strong>
                      <span className="plugin-pill">v{plugin.version}</span>
                      {!plugin.approved && <span className="plugin-pill warn">待授权</span>}
                      {!plugin.backendAvailable && (
                        <span className="plugin-pill warn">当前平台无后端</span>
                      )}
                    </div>
                    <p>{plugin.description || '第三方插件'}</p>
                    <div className="plugin-row-meta">
                      <span>{plugin.author?.name || '未知开发者'}</span>
                      <span aria-hidden>·</span>
                      <span>{declared.length ? `${declared.length} 项权限` : '无需宿主权限'}</span>
                      {plugin.contributes?.connectionFactories?.length ? (
                        <>
                          <span aria-hidden>·</span>
                          <span>连接提供者</span>
                        </>
                      ) : null}
                    </div>
                  </div>
                  <div className="plugin-row-actions">
                    <div className="plugin-row-buttons">
                      {hasSettings && (
                        <Button
                          type="button"
                          size="icon-sm"
                          variant="ghost"
                          aria-label={`设置 ${plugin.name}`}
                          title="插件设置"
                          onClick={() => {
                            setMessage('')
                            setSettingsFor(plugin)
                          }}
                        >
                          <Settings2 size={15} />
                        </Button>
                      )}
                      <Button
                        type="button"
                        size="icon-sm"
                        variant="ghost"
                        aria-label="权限管理"
                        title="权限管理"
                        onClick={() => {
                          setMessage('')
                          setPermissionsFor(plugin)
                        }}
                      >
                        <ShieldCheck size={14} />
                      </Button>
                    </div>
                    <button
                      type="button"
                      role="switch"
                      aria-checked={enabled}
                      aria-label={`${enabled ? '停用' : '启用'} ${plugin.name}`}
                      className={`plugin-switch ${enabled ? 'on' : ''}`}
                      disabled={busy || !plugin.approved}
                      onClick={() => void updateActivation(plugin, !enabled)}
                    >
                      <span />
                    </button>
                  </div>
                </article>
              )
            })}
          </div>
        ) : (
          <div className="plugin-empty">
            <div className="plugin-empty-icon">
              <Puzzle size={26} />
            </div>
            <strong>用插件扩展 Fiex</strong>
            <p>Git、WebDAV、SMB、压缩与校验和都以可选插件提供，不会预装。</p>
            <Button type="button" onClick={() => setStoreOpen(true)}>
              <Download size={15} />
              浏览官方插件
            </Button>
          </div>
        )}

        {message && (
          <p role="status" className="plugin-page-message">
            {message}
          </p>
        )}
      </div>

      <PluginInstaller
        open={storeOpen}
        installedIds={plugins.map((plugin) => plugin.id)}
        onClose={() => setStoreOpen(false)}
        onInstalled={() => {
          refreshPluginCatalog()
          setMessage('插件已安装。你可以在列表中查看权限或停用。')
        }}
      />

      <Modal
        open={Boolean(permissionsFor)}
        onClose={() => {
          setPermissionsFor(null)
          setMessage('')
        }}
        title={permissionsFor ? `${permissionsFor.name} · 权限` : '插件权限'}
        wide
        className="plugin-permissions-modal"
      >
        {permissionsFor && (
          <div className="plugin-permissions-page">
            <div className="plugin-detail-heading">
              <div className="plugin-row-icon enabled">{iconFor(permissionsFor.id)}</div>
              <div>
                <strong>{permissionsFor.name}</strong>
                <small>
                  {permissionsFor.id} · v{permissionsFor.version}
                </small>
                <p>{permissionsFor.description || '没有提供说明。'}</p>
              </div>
            </div>
            {!permissionsFor.approved && (
              <div className="plugin-consent-banner">
                <ShieldCheck size={18} />
                <div>
                  <strong>此插件尚未授权</strong>
                  <p>安装时确认的权限会在下面列出。授权后才会启动插件进程。</p>
                </div>
                <Button type="button" size="sm" onClick={() => setConfirmInstall(true)}>
                  审核并启用
                </Button>
              </div>
            )}
            <div className="plugin-permission-list">
              <PermissionList
                plugin={permissionsFor}
                busy={busy}
                onToggle={(permission, granted) =>
                  void updatePermission(permissionsFor, permission, granted)
                }
              />
            </div>
            {permissionsFor.declaredPermissions.includes('network') && (
              <section className="plugin-domain-section">
                <div className="plugin-section-title">
                  <strong>
                    <Globe2 size={15} /> 允许的域名
                  </strong>
                  <small>支持域名与 IP；localhost / 回环地址始终允许。</small>
                </div>
                <div className="plugin-domain-list">
                  {[
                    ...new Set([
                      ...(permissionsFor.requestedNetworkDomains || []),
                      ...(permissionsFor.allowedNetworkDomains || []),
                    ]),
                  ]
                    .sort()
                    .map((domain) => {
                      const allowed = (permissionsFor.allowedNetworkDomains || []).includes(domain)
                      const requested = (permissionsFor.requestedNetworkDomains || []).includes(
                        domain,
                      )
                      return (
                        <div className="plugin-domain-row" key={domain}>
                          <span>
                            {domain}
                            {requested && <small>插件申请</small>}
                            {!allowed && <small className="muted">未授权</small>}
                          </span>
                          <Button
                            type="button"
                            size="icon-xs"
                            variant="ghost"
                            aria-label={allowed ? `移除域名 ${domain}` : `授权域名 ${domain}`}
                            title={allowed ? '移除域名' : '授权域名'}
                            disabled={
                              busy || !permissionsFor.grantedPermissions.includes('network')
                            }
                            onClick={() => void updateDomain(permissionsFor, domain, !allowed)}
                          >
                            {allowed ? <X size={13} /> : <Plus size={13} />}
                          </Button>
                        </div>
                      )
                    })}
                  {![
                    ...(permissionsFor.requestedNetworkDomains || []),
                    ...(permissionsFor.allowedNetworkDomains || []),
                  ].length && <p className="sidebar-hint">尚未允许任何域名。</p>}
                </div>
                <div className="plugin-domain-add">
                  <input
                    aria-label="添加允许的域名"
                    placeholder="域名、IP 或 gitea.example.com"
                    value={domainDraft}
                    disabled={busy || !permissionsFor.grantedPermissions.includes('network')}
                    onChange={(event) => setDomainDraft(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') {
                        const domain = domainDraft.trim()
                        if (domain) void updateDomain(permissionsFor, domain, true)
                      }
                    }}
                  />
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={
                      busy ||
                      !domainDraft.trim() ||
                      !permissionsFor.grantedPermissions.includes('network')
                    }
                    onClick={() => {
                      const domain = domainDraft.trim()
                      if (domain) void updateDomain(permissionsFor, domain, true)
                    }}
                  >
                    添加域名
                  </Button>
                </div>
              </section>
            )}
            <div className="plugin-permission-actions">
              <p>
                撤销权限会重启插件进程。插件数据和同步目录不会被删除。
                <br />
                当前状态：{permissionsFor.enabled ? '已启用' : '已停用'} ·{' '}
                {permissionsFor.approved ? '已授权' : '待授权'}
              </p>
              <div>
                <Button
                  type="button"
                  variant="outline"
                  disabled={busy}
                  onClick={() => setConfirmRemove(permissionsFor)}
                >
                  <Trash2 size={14} />
                  卸载插件
                </Button>
                <Button
                  type="button"
                  disabled={busy}
                  onClick={() => void updateActivation(permissionsFor, !permissionsFor.enabled)}
                >
                  {permissionsFor.enabled ? '停用插件' : '启用插件'}
                </Button>
              </div>
            </div>
            {message && (
              <p role="status" className="plugin-page-message">
                {message}
              </p>
            )}
          </div>
        )}
      </Modal>

      <Modal
        open={Boolean(settingsFor)}
        onClose={() => {
          setSettingsFor(null)
          setMessage('')
        }}
        title={settingsFor ? `${settingsFor.name} · 设置` : '插件设置'}
      >
        <div className="plugin-settings-fields">
          {fields.map((field: PluginField) => (
            <label key={field.id} className="plugin-settings-field">
              <span>
                {field.type === 'secret' ? <KeyRound size={14} /> : <Database size={14} />}
                {field.label}
              </span>
              {field.type === 'checkbox' ? (
                <input
                  type="checkbox"
                  checked={Boolean(draft[field.id])}
                  onChange={(event) =>
                    setDraft((old) => ({ ...old, [field.id]: event.target.checked }))
                  }
                />
              ) : (
                <input
                  type={field.type === 'secret' ? 'password' : field.type}
                  value={String(draft[field.id] ?? '')}
                  placeholder={
                    field.type === 'secret' ? '保留原有凭据（留空不修改）' : field.placeholder
                  }
                  onChange={(event) =>
                    setDraft((old) => ({ ...old, [field.id]: event.target.value }))
                  }
                />
              )}
            </label>
          ))}
          {!fields.length && <p className="sidebar-hint">此插件没有需要配置的设置。</p>}
          <p className="sidebar-hint">
            <KeyRound size={13} /> 敏感凭据由系统凭据库管理，不会回显。
          </p>
          {message && <p role="status">{message}</p>}
          <footer className="plugin-consent-actions">
            <Button variant="outline" onClick={() => setSettingsFor(null)}>
              关闭
            </Button>
            <Button disabled={busy || !fields.length} onClick={() => void saveSettings()}>
              {busy ? <LoaderCircle className="spin" size={14} /> : <Check size={14} />}
              保存插件设置
            </Button>
          </footer>
        </div>
      </Modal>

      <Modal
        open={Boolean(domainToApprove)}
        title="允许插件访问网络域名"
        onClose={() => setDomainToApprove(null)}
      >
        <p>
          确认允许「{permissionsFor?.name}」访问 <strong>{domainToApprove}</strong>{' '}
          吗？仅授权此精确域名，不包括子域名。
        </p>
        <footer className="plugin-consent-actions">
          <Button variant="outline" onClick={() => setDomainToApprove(null)}>
            取消
          </Button>
          <Button
            disabled={busy}
            onClick={() =>
              permissionsFor &&
              domainToApprove &&
              void updateDomain(permissionsFor, domainToApprove, true)
            }
          >
            允许此域名
          </Button>
        </footer>
      </Modal>
      <Modal
        open={Boolean(permissionToApprove)}
        title="重新授予插件权限"
        onClose={() => setPermissionToApprove(null)}
      >
        <p>
          允许「{permissionsFor?.name}」重新获得
          {permissionLabels[permissionToApprove || ''] || permissionToApprove}权限吗？
        </p>
        <footer className="plugin-consent-actions">
          <Button variant="outline" onClick={() => setPermissionToApprove(null)}>
            取消
          </Button>
          <Button
            disabled={busy}
            onClick={() => permissionsFor && void grantPermission(permissionsFor)}
          >
            允许
          </Button>
        </footer>
      </Modal>
      <Modal open={Boolean(confirmRemove)} title="卸载插件" onClose={() => setConfirmRemove(null)}>
        <p>
          确定卸载「{confirmRemove?.name}
          」吗？将停止插件进程并删除程序文件，插件数据和用户同步目录会保留。
        </p>
        <footer className="plugin-consent-actions">
          <Button variant="outline" onClick={() => setConfirmRemove(null)}>
            取消
          </Button>
          <Button variant="destructive" disabled={busy} onClick={() => void uninstall()}>
            卸载
          </Button>
        </footer>
      </Modal>
      <Modal
        open={confirmInstall}
        title="安装插件权限确认"
        onClose={() => setConfirmInstall(false)}
      >
        <p>「{permissionsFor?.name}」申请以下权限，确认后一次性授权。此后可在设置中撤销。</p>
        <ul>
          {(permissionsFor?.declaredPermissions || []).map((permission) => (
            <li key={permission}>{permissionLabels[permission] || permission}</li>
          ))}
        </ul>
        <footer className="plugin-consent-actions">
          <Button variant="outline" onClick={() => setConfirmInstall(false)}>
            取消
          </Button>
          <Button disabled={busy} onClick={() => permissionsFor && void approve(permissionsFor)}>
            同意并启用
          </Button>
        </footer>
      </Modal>
    </>
  )
}

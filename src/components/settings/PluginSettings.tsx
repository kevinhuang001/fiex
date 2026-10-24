import { useEffect, useState } from 'react'
import {
  Check,
  Database,
  Download,
  Globe2,
  KeyRound,
  LoaderCircle,
  Puzzle,
  Settings2,
  ShieldAlert,
  ShieldCheck,
  Terminal,
  Trash2,
  Upload,
  AlertTriangle,
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
import { PluginInstaller, pluginIcon } from './PluginInstaller'
import { PluginActivationDialog } from './PluginActivationDialog'
import { PluginPermissionPrompt, promptForPermission } from './PluginPermissionPrompt'
import { permissionHints, permissionLabels } from '@/lib/pluginPermissions'

interface LocalPreview {
  ticket: string
  id: string
  name: string
  version: string
  author?: { name: string }
  permissions: string[]
  requestedNetworkDomains: string[]
  commands: string[]
  signed: boolean
  trusted: boolean
  publisher?: string
  publisherName?: string
  keyId?: string
  fingerprint?: string
}

function iconFor(icon?: string) {
  return pluginIcon(icon)
}

function PermissionList({
  plugin,
  busy,
  onToggle,
  onConfigure,
}: {
  plugin: PluginInfo
  busy: boolean
  onToggle: (permission: string, granted: boolean) => void
  onConfigure: (permission: string) => void
}) {
  const declared = plugin.declaredPermissions || []
  const granted = plugin.grantedPermissions || []
  const configurable = (permission: string) => permission === 'network' || permission === 'commands'
  return (
    <>
      {declared.map((permission) => (
        <div className="plugin-permission-card" key={permission}>
          <span className="plugin-permission-icon">
            <ShieldCheck size={15} />
          </span>
          <span className="plugin-permission-copy">
            <strong>{permissionLabels[permission] || permission}</strong>
            <small>{permissionHints[permission] || permission}</small>
            {permission === 'commands' && Boolean(plugin.missingCommands?.length) && (
              <small className="plugin-command-missing">
                本地未找到：{(plugin.missingCommands || []).join('、')}，插件可能无法正常工作
              </small>
            )}
          </span>
          {configurable(permission) && (
            <button
              type="button"
              className="plugin-inline-config"
              aria-label={permission === 'network' ? '配置网络域名' : '配置允许的命令'}
              title={permission === 'network' ? '配置网络域名' : '配置允许的命令'}
              disabled={busy || !plugin.approved || !granted.includes(permission)}
              onClick={() => onConfigure(permission)}
            >
              {permission === 'network' ? <Globe2 size={13} /> : <Terminal size={13} />}
            </button>
          )}
          <button
            type="button"
            role="switch"
            aria-checked={granted.includes(permission)}
            aria-label={`授权 ${permissionLabels[permission] || permission}`}
            className={`plugin-switch ${granted.includes(permission) ? 'on' : ''}`}
            disabled={busy || !plugin.approved}
            onClick={() => onToggle(permission, !granted.includes(permission))}
          >
            <span />
          </button>
        </div>
      ))}
      {!declared.length && <p className="sidebar-hint">此插件未声明受控宿主权限。</p>}
    </>
  )
}

/** Settings stored before `list` existed may be a comma/newline separated
 * string; accept both shapes and always hand the editor a string array. */
function asList(value: unknown, field: PluginField): string[] {
  const fallback = field.defaultValue
  if (Array.isArray(value)) return value.map((item) => String(item))
  if (typeof value === 'string' && value.trim())
    return value
      .split(/[\n,，]/)
      .map((item) => item.trim())
      .filter(Boolean)
  if (Array.isArray(fallback)) return fallback.map((item) => String(item))
  if (typeof fallback === 'string' && fallback.trim())
    return fallback
      .split(/[\n,，]/)
      .map((item) => item.trim())
      .filter(Boolean)
  return []
}

function PluginListField({
  field,
  values,
  onChange,
}: {
  field: PluginField
  values: string[]
  onChange: (values: string[]) => void
}) {
  const update = (index: number, value: string) =>
    onChange(values.map((item, i) => (i === index ? value : item)))
  return (
    <div className="plugin-list-field">
      <div className="plugin-list-rows">
        {values.map((value, index) => (
          <div className="plugin-list-row" key={index}>
            <input
              aria-label={`${field.label} 第 ${index + 1} 项`}
              value={value}
              placeholder={field.placeholder}
              onChange={(event) => update(index, event.target.value)}
            />
            <button
              type="button"
              className="plugin-list-remove"
              aria-label={`删除${field.label}第 ${index + 1} 项`}
              title="删除"
              onClick={() => onChange(values.filter((_, i) => i !== index))}
            >
              <Trash2 size={13} />
            </button>
          </div>
        ))}
        {!values.length && <p className="plugin-list-empty">还没有条目。</p>}
      </div>
      <Button type="button" size="sm" variant="outline" onClick={() => onChange([...values, ''])}>
        添加条目
      </Button>
    </div>
  )
}

type PluginError = {
  at: number
  source: string
  message: string
  detail?: string
}

/** Recent failures Core observed or the plugin reported. Diagnostics only: it
 * never blocks the plugin, and "clear" only empties this log. */
function PluginErrorsSection({ pluginId }: { pluginId: string }) {
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
export function PluginSettings() {
  const plugins = usePluginCatalog()
  const [storeOpen, setStoreOpen] = useState(false)
  const [localPreview, setLocalPreview] = useState<LocalPreview | null>(null)
  const [permissionsFor, setPermissionsFor] = useState<PluginInfo | null>(null)
  const [settingsFor, setSettingsFor] = useState<PluginInfo | null>(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [draft, setDraft] = useState<Record<string, string | boolean | string[]>>({})
  const [permissionToApprove, setPermissionToApprove] = useState<string | null>(null)
  useEffect(() => {
    setPermissionsFor((current) =>
      current ? plugins.find((plugin) => plugin.id === current.id) || null : null,
    )
    setSettingsFor((current) =>
      current ? plugins.find((plugin) => plugin.id === current.id) || null : null,
    )
  }, [plugins])
  const [domainDraft, setDomainDraft] = useState('')
  const [domainSettingsOpen, setDomainSettingsOpen] = useState(false)
  const [domainToApprove, setDomainToApprove] = useState<string | null>(null)
  const [commandDraft, setCommandDraft] = useState('')
  const [commandSettingsOpen, setCommandSettingsOpen] = useState(false)
  const [activationFor, setActivationFor] = useState<PluginInfo | null>(null)
  const [commandToApprove, setCommandToApprove] = useState<string | null>(null)
  const [confirmRemove, setConfirmRemove] = useState<PluginInfo | null>(null)

  const chooseLocalPlugin = async () => {
    setBusy(true)
    setMessage('')
    try {
      const path = await call<string | null>('plugin_file_dialog', {
        mode: 'openFile',
        extensions: ['fiexplugin'],
      })
      if (!path) return
      const preview = await call<LocalPreview>('plugin_prepare_local_install', { path })
      setLocalPreview(preview)
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const installLocal = async () => {
    if (!localPreview) return
    setBusy(true)
    try {
      await call('plugin_confirm_install', {
        ticket: localPreview.ticket,
        allowUntrusted: !localPreview.trusted,
      })
      setLocalPreview(null)
      refreshPluginCatalog()
      setMessage(`已导入 ${localPreview.name} ${localPreview.version}`)
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }

  const fields = settingsFor?.contributions?.settings || []
  useEffect(() => {
    if (!settingsFor) return
    let alive = true
    setMessage('')
    void Promise.all(
      fields.map(async (field) => {
        if (field.type === 'secret') return [field.id, ''] as const
        const value = await call<unknown>('plugin_storage_get', {
          pluginId: settingsFor.id,
          scope: 'global',
          key: field.id,
        })
        if (field.type === 'list') return [field.id, asList(value, field)] as const
        return [
          field.id,
          (value ?? field.defaultValue ?? (field.type === 'checkbox' ? false : '')) as
            string | boolean,
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
    if (enabled) {
      setActivationFor(plugin)
      return
    }
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
  const updateCommand = async (plugin: PluginInfo, command: string, granted: boolean) => {
    if (granted && commandToApprove !== command) {
      setCommandToApprove(command)
      return
    }
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_command', { pluginId: plugin.id, command, granted })
      setCommandDraft('')
      setCommandToApprove(null)
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
      setMessage('')
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
            value:
              field.type === 'number'
                ? Number(value)
                : field.type === 'list'
                  ? (Array.isArray(value) ? value : []).map((item) => item.trim()).filter(Boolean)
                  : value,
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
        <div className="plugin-toolbar plugin-toolbar-actions-only">
          <div className="plugin-toolbar-actions">
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => void chooseLocalPlugin()}
            >
              <Upload size={15} />
              导入插件
            </Button>
            <Button type="button" size="sm" onClick={() => setStoreOpen(true)}>
              <Download size={15} />
              获取插件
            </Button>
          </div>
        </div>

        {plugins.length ? (
          <div className="plugin-list">
            {plugins.map((plugin) => {
              const declared = plugin.declaredPermissions || []
              const enabled = plugin.enabled && plugin.approved
              const hasSettings = (plugin.contributions?.settings || []).length > 0
              return (
                <article className="plugin-row" key={plugin.id}>
                  <div className={`plugin-row-icon ${enabled ? 'enabled' : ''}`}>
                    {iconFor(plugin.icon)}
                  </div>
                  <div className="plugin-row-main">
                    <div className="plugin-row-title">
                      <strong>{plugin.name}</strong>
                      <span className="plugin-pill">v{plugin.version}</span>
                      <span className={`plugin-state ${enabled ? 'active' : ''}`}>
                        <span />
                        {enabled ? '运行中' : '未激活'}
                      </span>
                      {!plugin.backendAvailable && (
                        <span className="plugin-pill warn">当前平台无后端</span>
                      )}
                    </div>
                    <p>{plugin.description || '第三方插件'}</p>
                    <div className="plugin-row-meta">
                      <span>{plugin.author?.name || '未知开发者'}</span>
                      <span aria-hidden>·</span>
                      <span>{declared.length ? `${declared.length} 项权限` : '无需宿主权限'}</span>
                      {plugin.contributions?.connections?.length ? (
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
                        aria-label="插件详情"
                        title="插件详情"
                        onClick={() => {
                          setMessage('')
                          setDomainSettingsOpen(false)
                          setCommandSettingsOpen(false)
                          setPermissionsFor(plugin)
                        }}
                      >
                        <ShieldCheck size={14} />
                      </Button>
                      <Button
                        type="button"
                        size="icon-sm"
                        variant="ghost"
                        className="plugin-row-remove"
                        aria-label={`卸载 ${plugin.name}`}
                        title="卸载插件"
                        disabled={busy}
                        onClick={() => {
                          setMessage('')
                          setConfirmRemove(plugin)
                        }}
                      >
                        <Trash2 size={14} />
                      </Button>
                    </div>
                    <button
                      type="button"
                      role="switch"
                      aria-checked={enabled}
                      aria-label={`${enabled ? '停用' : '激活'} ${plugin.name}`}
                      className={`plugin-switch ${enabled ? 'on' : ''}`}
                      disabled={busy || !plugin.backendAvailable}
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
            <p>按需添加连接、文件工具和工作流，所有插件都需要你的授权。</p>
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
          setMessage('插件已安装，尚未激活。点击激活后逐项审核权限。')
        }}
      />

      <Modal
        open={Boolean(permissionsFor)}
        onClose={() => {
          setPermissionsFor(null)
          setDomainSettingsOpen(false)
          setCommandSettingsOpen(false)
          setMessage('')
        }}
        title="插件详情"
        className="plugin-permissions-modal"
      >
        {permissionsFor && (
          <div className="plugin-permissions-page">
            <div className="plugin-permission-profile">
              <div
                className={`plugin-permission-profile-icon ${permissionsFor.enabled ? 'enabled' : ''}`}
              >
                {iconFor(permissionsFor.icon)}
              </div>
              <div className="plugin-permission-profile-copy">
                <div className="plugin-permission-profile-title">
                  <strong>{permissionsFor.name}</strong>
                  <span className={`plugin-state ${permissionsFor.enabled ? 'active' : ''}`}>
                    <span />
                    {permissionsFor.enabled ? '运行中' : '未激活'}
                  </span>
                  {!permissionsFor.approved && <span className="plugin-pill warn">待授权</span>}
                </div>
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
                  <p>激活时会逐项请求权限，允许后才会运行。</p>
                </div>
                <Button type="button" size="sm" onClick={() => setActivationFor(permissionsFor)}>
                  审核并激活
                </Button>
              </div>
            )}
            <section className="plugin-permission-section">
              <div className="plugin-section-title">
                <strong>
                  <ShieldCheck size={15} /> 权限
                </strong>
                <small>
                  {(permissionsFor.grantedPermissions || []).length} /{' '}
                  {(permissionsFor.declaredPermissions || []).length} 已授权
                </small>
              </div>
              <div className="plugin-permission-list">
                <PermissionList
                  plugin={permissionsFor}
                  busy={busy}
                  onToggle={(permission, granted) =>
                    void updatePermission(permissionsFor, permission, granted)
                  }
                  onConfigure={(permission) => {
                    setMessage('')
                    if (permission === 'network') setDomainSettingsOpen(true)
                    else setCommandSettingsOpen(true)
                  }}
                />
              </div>
            </section>
            <PluginErrorsSection pluginId={permissionsFor.id} />
            {message && (
              <p role="status" className="plugin-page-message">
                {message}
              </p>
            )}
          </div>
        )}
      </Modal>

      {permissionsFor &&
        domainSettingsOpen &&
        permissionsFor.declaredPermissions.includes('network') && (
          <Modal
            open
            onClose={() => setDomainSettingsOpen(false)}
            title="网络域名"
            className="plugin-domain-modal"
          >
            <div className="plugin-domain-page">
              <p className="plugin-domain-hint">
                允许「{permissionsFor.name}」访问的精确主机；本机和局域网地址也需要授权。
              </p>
              <section className="plugin-permission-section plugin-domain-section">
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
                          <button
                            type="button"
                            role="switch"
                            aria-checked={allowed}
                            aria-label={allowed ? `移除域名 ${domain}` : `授权域名 ${domain}`}
                            title={allowed ? '移除域名' : '授权域名'}
                            className={`plugin-switch ${allowed ? 'on' : ''}`}
                            disabled={
                              busy || !permissionsFor.grantedPermissions.includes('network')
                            }
                            onClick={() => void updateDomain(permissionsFor, domain, !allowed)}
                          >
                            <span />
                          </button>
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
            </div>
          </Modal>
        )}
      {permissionsFor &&
        commandSettingsOpen &&
        permissionsFor.declaredPermissions.includes('commands') && (
          <Modal
            open
            onClose={() => setCommandSettingsOpen(false)}
            title="外部命令"
            className="plugin-domain-modal"
          >
            <div className="plugin-domain-page">
              <p className="plugin-domain-hint">
                外部命令由宿主从 PATH 解析执行，可能访问授权目录以外的资源。
              </p>
              <section className="plugin-permission-section plugin-domain-section">
                <div className="plugin-domain-list">
                  {[
                    ...new Set([
                      ...(permissionsFor.requestedCommands || []),
                      ...(permissionsFor.allowedCommands || []),
                    ]),
                  ]
                    .sort()
                    .map((command) => {
                      const allowed = (permissionsFor.allowedCommands || []).includes(command)
                      const requested = (permissionsFor.requestedCommands || []).includes(command)
                      const missing = (permissionsFor.missingCommands || []).includes(command)
                      return (
                        <div
                          className={`plugin-domain-row ${missing ? 'plugin-command-row-missing' : ''}`}
                          key={command}
                        >
                          <span>
                            <code>{command}</code>
                            {requested && <small>插件申请</small>}
                            {!allowed && <small className="muted">未授权</small>}
                            {missing && (
                              <small className="plugin-command-missing">本地未找到</small>
                            )}
                          </span>
                          <button
                            type="button"
                            role="switch"
                            aria-checked={allowed}
                            aria-label={allowed ? `移除命令 ${command}` : `授权命令 ${command}`}
                            title={allowed ? '移除命令' : '授权命令'}
                            className={`plugin-switch ${allowed ? 'on' : ''}`}
                            disabled={
                              busy || !permissionsFor.grantedPermissions.includes('commands')
                            }
                            onClick={() => void updateCommand(permissionsFor, command, !allowed)}
                          >
                            <span />
                          </button>
                        </div>
                      )
                    })}
                  {![
                    ...(permissionsFor.requestedCommands || []),
                    ...(permissionsFor.allowedCommands || []),
                  ].length && <p className="sidebar-hint">尚未允许任何命令。</p>}
                </div>
                <div className="plugin-domain-add">
                  <input
                    aria-label="添加允许的命令"
                    placeholder="输入可执行文件名称"
                    value={commandDraft}
                    disabled={busy || !permissionsFor.grantedPermissions.includes('commands')}
                    onChange={(event) => setCommandDraft(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') {
                        const command = commandDraft.trim()
                        if (command) void updateCommand(permissionsFor, command, true)
                      }
                    }}
                  />
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={
                      busy ||
                      !commandDraft.trim() ||
                      !permissionsFor.grantedPermissions.includes('commands')
                    }
                    onClick={() => {
                      const command = commandDraft.trim()
                      if (command) void updateCommand(permissionsFor, command, true)
                    }}
                  >
                    添加命令
                  </Button>
                </div>
              </section>
            </div>
          </Modal>
        )}
      <Modal
        open={Boolean(settingsFor)}
        onClose={() => {
          setSettingsFor(null)
          setMessage('')
        }}
        title={settingsFor ? `${settingsFor.name} · 设置` : '插件设置'}
      >
        <div className="plugin-settings-fields">
          {fields.map((field: PluginField) => {
            const label = (
              <span className="plugin-settings-label">
                {field.type === 'secret' ? <KeyRound size={14} /> : <Database size={14} />}
                {field.label}
              </span>
            )
            if (field.type === 'list')
              return (
                <div key={field.id} className="plugin-settings-field">
                  {label}
                  <PluginListField
                    field={field}
                    values={asList(draft[field.id], field)}
                    onChange={(values) => setDraft((old) => ({ ...old, [field.id]: values }))}
                  />
                </div>
              )
            return (
              <label key={field.id} className="plugin-settings-field">
                {label}
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
            )
          })}
          {!fields.length && <p className="sidebar-hint">此插件没有需要配置的设置。</p>}
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

      {permissionsFor && domainToApprove && (
        <PluginPermissionPrompt
          open
          pluginName={permissionsFor.name}
          icon={permissionsFor.icon}
          item={{
            id: `domain:${domainToApprove}`,
            kind: 'network',
            title: '网络域名',
            value: domainToApprove,
          }}
          step={1}
          total={1}
          busy={busy}
          onAllow={() => void updateDomain(permissionsFor, domainToApprove, true)}
          onDeny={() => setDomainToApprove(null)}
        />
      )}
      {permissionsFor && permissionToApprove && (
        <PluginPermissionPrompt
          open
          pluginName={permissionsFor.name}
          icon={permissionsFor.icon}
          item={promptForPermission(permissionToApprove)}
          step={1}
          total={1}
          busy={busy}
          onAllow={() => void grantPermission(permissionsFor)}
          onDeny={() => setPermissionToApprove(null)}
        />
      )}
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
        open={Boolean(localPreview)}
        title="导入插件"
        onClose={() => setLocalPreview(null)}
        className="plugin-import-modal"
      >
        {localPreview && (
          <div className="plugin-import-page">
            <div className="plugin-import-profile">
              <span className="plugin-import-icon">
                <Puzzle size={24} />
              </span>
              <span className="plugin-import-copy">
                <strong>{localPreview.name}</strong>
                <small>{localPreview.author?.name || '未知开发者'}</small>
              </span>
            </div>
            <div
              className={`plugin-import-signature ${
                localPreview.signed && localPreview.trusted ? 'verified' : 'warning'
              }`}
            >
              {localPreview.signed && localPreview.trusted ? (
                <>
                  <ShieldCheck size={16} />
                  <span>
                    <strong>签名已验证</strong>
                    {localPreview.publisherName || localPreview.publisher || '已验证发布者'}
                  </span>
                </>
              ) : (
                <>
                  <ShieldAlert size={16} />
                  <span>
                    <strong>{localPreview.signed ? '未信任的发布者' : '此插件未签名'}</strong>
                    {localPreview.signed
                      ? `${localPreview.publisherName || localPreview.publisher || '未知发布者'} · 密钥指纹 ${localPreview.fingerprint || localPreview.keyId || '未知'}`
                      : '只在你确认来源可信时继续导入。'}
                  </span>
                </>
              )}
            </div>
            <footer className="plugin-consent-actions">
              <Button
                variant="outline"
                disabled={busy}
                onClick={() => {
                  if (localPreview)
                    void call('plugin_cancel_install', { ticket: localPreview.ticket })
                  setLocalPreview(null)
                }}
              >
                取消
              </Button>
              <Button disabled={busy} onClick={() => void installLocal()}>
                导入插件
              </Button>
            </footer>
          </div>
        )}
      </Modal>
      {activationFor && (
        <PluginActivationDialog
          key={activationFor.id}
          plugin={activationFor}
          onClose={() => setActivationFor(null)}
          onActivated={() => {
            setActivationFor(null)
            refreshPluginCatalog()
            setMessage('插件已激活，可随时在插件详情中撤销授权。')
          }}
        />
      )}
      {permissionsFor && commandToApprove && (
        <PluginPermissionPrompt
          open
          pluginName={permissionsFor.name}
          icon={permissionsFor.icon}
          item={{
            id: `command:${commandToApprove}`,
            kind: 'command',
            title: '外部命令',
            value: commandToApprove,
          }}
          step={1}
          total={1}
          busy={busy}
          onAllow={() => void updateCommand(permissionsFor, commandToApprove, true)}
          onDeny={() => setCommandToApprove(null)}
        />
      )}
    </>
  )
}

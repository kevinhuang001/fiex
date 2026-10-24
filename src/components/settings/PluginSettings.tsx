import { useEffect, useState } from 'react'
import { Download, Puzzle, Settings2, ShieldAlert, ShieldCheck, Trash2, Upload } from 'lucide-react'
import { call } from '@/lib/api'
import { asList } from './plugins/listValue'
import { CommandDialog, DomainDialog } from './plugins/PermissionDialogs'
import { PluginPermissionsSheet } from './plugins/PluginPermissionsSheet'
import { PluginSettingsSheet } from './plugins/PluginSettingsSheet'
import { refreshPluginCatalog, usePluginCatalog, type PluginInfo } from '@/lib/plugins'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import { PluginInstaller, pluginIcon } from './PluginInstaller'
import { PluginActivationDialog } from './PluginActivationDialog'
import { PluginPermissionPrompt, promptForPermission } from './PluginPermissionPrompt'

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
  const updateDraft = (fieldId: string, value: string | boolean | string[]) =>
    setDraft((old) => ({ ...old, [fieldId]: value }))

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
                    {pluginIcon(plugin.icon)}
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

      <PluginPermissionsSheet
        plugin={permissionsFor}
        busy={busy}
        message={message}
        onClose={() => {
          setPermissionsFor(null)
          setDomainSettingsOpen(false)
          setCommandSettingsOpen(false)
          setMessage('')
        }}
        onActivate={setActivationFor}
        onTogglePermission={(plugin, permission, granted) =>
          void updatePermission(plugin, permission, granted)
        }
        onConfigure={(permission) => {
          setMessage('')
          if (permission === 'network') setDomainSettingsOpen(true)
          else setCommandSettingsOpen(true)
        }}
      />

      {permissionsFor &&
        domainSettingsOpen &&
        permissionsFor.declaredPermissions.includes('network') && (
          <DomainDialog
            plugin={permissionsFor}
            busy={busy}
            draft={domainDraft}
            onDraftChange={setDomainDraft}
            onToggle={(domain, granted) => void updateDomain(permissionsFor, domain, granted)}
            onAdd={(domain) => void updateDomain(permissionsFor, domain, true)}
            onClose={() => setDomainSettingsOpen(false)}
          />
        )}
      {permissionsFor &&
        commandSettingsOpen &&
        permissionsFor.declaredPermissions.includes('commands') && (
          <CommandDialog
            plugin={permissionsFor}
            busy={busy}
            draft={commandDraft}
            onDraftChange={setCommandDraft}
            onToggle={(command, granted) => void updateCommand(permissionsFor, command, granted)}
            onAdd={(command) => void updateCommand(permissionsFor, command, true)}
            onClose={() => setCommandSettingsOpen(false)}
          />
        )}

      <PluginSettingsSheet
        plugin={settingsFor}
        fields={fields}
        draft={draft}
        busy={busy}
        message={message}
        onDraftChange={updateDraft}
        onSave={() => void saveSettings()}
        onClose={() => setSettingsFor(null)}
        onDismiss={() => {
          setSettingsFor(null)
          setMessage('')
        }}
      />

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

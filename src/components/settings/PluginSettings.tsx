import { useEffect, useState } from 'react'
import { Database, KeyRound, Puzzle, ShieldCheck } from 'lucide-react'
import { call } from '@/lib/api'
import { usePluginCatalog, refreshPluginCatalog, type PluginField } from '@/lib/plugins'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'

const permissionLabels: Record<string,string> = {
  storage: '插件私有数据存储',
  secrets: '系统凭据库（密码、OAuth 令牌）',
  network: '网络访问',
  'filesystem.read': '读取用户文件',
  'filesystem.write': '修改用户文件',
  externalCommands: '执行外部程序',
  'ui.dialog': '打开文件与文件夹选择器',
}
export function PluginSettings() {
  const plugins = usePluginCatalog()
  const [selected, setSelected] = useState('')
  const [draft, setDraft] = useState<Record<string, string | boolean>>({})
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [permissionToApprove, setPermissionToApprove] = useState<string | null>(null)
  const [confirmInstall, setConfirmInstall] = useState(false)
  const active = plugins.find((p) => p.id === selected) || plugins[0]
  const fields = active?.contributes?.settings || []

  useEffect(() => {
    if (!active) return
    let alive = true
    setMessage('')
    void Promise.all(
      fields.map(async (field) => {
        if (field.type === 'secret') return [field.id, ''] as const
        const value = await call<string | boolean | null>('plugin_storage_get', {
          pluginId: active.id,
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
  }, [active?.id])

  const updateActivation = async (enabled: boolean) => {
    if (!active) return
    setBusy(true)
    try {
      await call('plugin_set_enabled', { pluginId: active.id, enabled })
      refreshPluginCatalog()
    } catch (error) { setMessage(String(error)) }
    finally { setBusy(false) }
  }
  const updatePermission = async (permission: string, granted: boolean) => {
    if (!active) return
    if (granted && !permissionToApprove) {
      setPermissionToApprove(permission)
      return
    }
    setBusy(true)
    try {
      await call('plugin_set_permission', { pluginId: active.id, permission, granted })
      setPermissionToApprove(null)
      refreshPluginCatalog()
    } catch (error) { setMessage(String(error)) }
    finally { setBusy(false) }
  }
  const install = async () => {
    if (!active) return
    setBusy(true)
    try {
      await call('plugin_approve_install', { pluginId: active.id })
      setConfirmInstall(false)
      refreshPluginCatalog()
    } catch (error) { setMessage(String(error)) }
    finally { setBusy(false) }
  }
  const save = async () => {
    if (!active || busy) return
    setBusy(true)
    setMessage('')
    try {
      for (const field of fields) {
        const value = draft[field.id]
        if (field.type === 'secret') {
          if (typeof value === 'string' && value.length)
            await call('plugin_secret_set', {
              pluginId: active.id,
              scope: 'global',
              key: field.id,
              value,
            })
        } else {
          await call('plugin_storage_set', {
            pluginId: active.id,
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
    <div className="plugin-settings-layout">
      <header className="settings-section-heading">
        <Puzzle size={19} />
        <div>
          <h3>插件</h3>
          <p>管理已安装的插件及其设置</p>
        </div>
      </header>
      <div className="plugin-settings-grid">
        <div className="plugin-settings-list">
          {plugins.map((plugin) => (
            <button
              key={plugin.id}
              className={active?.id === plugin.id ? 'active' : ''}
              onClick={() => setSelected(plugin.id)}
            >
              <Puzzle size={16} />
              <span>
                <strong>{plugin.name}</strong>
                <small>{plugin.version}</small>
              </span>
            </button>
          ))}
          {!plugins.length && <p className="sidebar-hint">尚未安装任何插件</p>}
        </div>
        <div className="plugin-settings-detail">
          {active ? (
            <>
              <strong>{active.name}</strong>
              <small>
                {active.id} · {active.version}
              </small>
              <p className="sidebar-hint">{active.capabilities.join(' · ')}</p>
              <div className="plugin-permission-heading">
                <strong>{active.official ? 'Fiex 官方插件' : '第三方插件'}</strong>
                <label>
                  <span>启用插件</span>
                  <input aria-label="启用插件" type="checkbox" checked={active.enabled}
                    disabled={busy || !active.approved}
                    onChange={event => void updateActivation(event.target.checked)} />
                </label>
              </div>
              {!active.approved && !active.official && (
                <div className="plugin-install-consent">
                  <p>此插件尚未授权。安装时确认声明的权限后即可启用。</p>
                  <Button type="button" disabled={busy} onClick={() => setConfirmInstall(true)}>审核并启用</Button>
                </div>
              )}
              <div className="plugin-permissions">
                <strong>权限管理</strong>
                <p className="sidebar-hint">官方插件首次默认同意声明的权限；关闭后不会自动恢复。重新开启权限需要确认。</p>
                {(active.declaredPermissions || []).map(permission => (
                  <label key={permission} className="plugin-permission-item">
                    <span>{permissionLabels[permission] || permission}</span>
                    <input type="checkbox" disabled={busy || !active.approved}
                      checked={active.enabled && active.grantedPermissions?.includes(permission)}
                      onChange={event => void updatePermission(permission, event.target.checked)} />
                  </label>
                ))}
                {!active.declaredPermissions?.length && <p className="sidebar-hint">此插件未声明受控宿主权限。</p>}
              </div>
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
                <ShieldCheck size={13} /> 敏感凭据由系统凭据库管理，不会回显。
              </p>
              {message && <p role="status">{message}</p>}
              <Button type="button" onClick={() => void save()} disabled={busy || !fields.length}>
                {busy ? '保存中…' : '保存插件设置'}
              </Button>
            </>
          ) : (
            <p className="sidebar-hint">选择一个插件查看设置</p>
          )}
        </div>
      </div>
    </div>
      <Modal open={Boolean(permissionToApprove)} title="重新授予插件权限" onClose={() => setPermissionToApprove(null)}>
        <p>允许「{active?.name}」重新获得{permissionLabels[permissionToApprove || ''] || permissionToApprove}权限吗？</p>
        <footer className="plugin-consent-actions">
          <Button variant="outline" onClick={() => setPermissionToApprove(null)}>取消</Button>
          <Button disabled={busy} onClick={() => permissionToApprove && void updatePermission(permissionToApprove, true)}>允许</Button>
        </footer>
      </Modal>
      <Modal open={confirmInstall} title="安装插件权限确认" onClose={() => setConfirmInstall(false)}>
        <p>「{active?.name}」申请以下权限，确认后一次性授权。此后可在设置中撤销。</p>
        <ul>{(active?.declaredPermissions || []).map(permission => <li key={permission}>{permissionLabels[permission] || permission}</li>)}</ul>
        <footer className="plugin-consent-actions">
          <Button variant="outline" onClick={() => setConfirmInstall(false)}>取消</Button>
          <Button disabled={busy} onClick={() => void install()}>同意并启用</Button>
        </footer>
      </Modal>
    </>
  )
}

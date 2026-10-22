import { useEffect, useState } from 'react'
import { Database, KeyRound, Puzzle, ShieldCheck } from 'lucide-react'
import { call } from '@/lib/api'
import { usePluginCatalog, type PluginField } from '@/lib/plugins'
import { Button } from '@/components/ui/button'

export function PluginSettings() {
  const plugins = usePluginCatalog()
  const [selected, setSelected] = useState('')
  const [draft, setDraft] = useState<Record<string, string | boolean>>({})
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
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
  )
}

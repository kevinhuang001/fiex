import { useEffect, useMemo, useState } from 'react'
import { ArrowLeft, Cloud, HardDrive, Plug, Server } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { call } from '@/lib/api'
import { usePluginCatalog, type PluginField } from '@/lib/plugins'
import type { RemoteConnection } from '@/lib/types'

export function ConnectionDialog({
  open,
  onClose,
  onSaved,
}: {
  open: boolean
  onClose: () => void
  onSaved: (connection: RemoteConnection) => void
}) {
  const plugins = usePluginCatalog()
  const choices = useMemo(
    () =>
      plugins.flatMap((plugin) =>
        plugin.enabled && plugin.capabilities.includes('connection')
          ? (plugin.contributions?.connections || []).map((factory) => ({
              plugin,
              factory,
            }))
          : [],
      ),
    [plugins],
  )
  const [selected, setSelected] = useState<string | null>(null)
  const [name, setName] = useState('')
  const [values, setValues] = useState<Record<string, string | boolean>>({})
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const current = choices.find(({ plugin, factory }) => plugin.id + ':' + factory.id === selected)

  useEffect(() => {
    if (!open) return
    setSelected(null)
    setName('')
    setValues({})
    setBusy(false)
    setError('')
  }, [open])

  const choose = (key: string, fields: PluginField[]) => {
    setSelected(key)
    setValues(
      Object.fromEntries(
        fields.map((field) => [
          field.id,
          field.defaultValue ?? (field.type === 'checkbox' ? false : ''),
        ]),
      ),
    )
    setError('')
  }
  const save = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!current || busy) return
    setBusy(true)
    setError('')
    try {
      const settings = Object.fromEntries(
        current.factory.fields.map((field) => [
          field.id,
          field.type === 'number'
            ? Number(values[field.id] || 0)
            : field.type === 'checkbox'
              ? Boolean(values[field.id])
              : String(values[field.id] ?? ''),
        ]),
      )
      const connection = await call<RemoteConnection>('save_remote_connection', {
        input: {
          name,
          pluginId: current.plugin.id,
          kind: current.factory.id,
          settings,
        },
      })
      onSaved(connection)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={current ? '配置 ' + current.plugin.name : '添加连接'}
    >
      {!current ? (
        <div className="connection-form">
          <p className="sidebar-hint">选择已安装的连接插件</p>
          <div className="connection-plugin-picker">
            {choices.map(({ plugin, factory }) => (
              <button
                key={plugin.id + ':' + factory.id}
                type="button"
                className="connection-plugin-choice"
                onClick={() => choose(plugin.id + ':' + factory.id, factory.fields)}
              >
                {factory.id === 'webdav' ? (
                  <Cloud size={20} />
                ) : factory.id === 'smb' ? (
                  <HardDrive size={20} />
                ) : (
                  <Plug size={20} />
                )}
                <span>
                  <strong>{factory.title || plugin.name}</strong>
                  <small>
                    {plugin.name} · {plugin.version}
                  </small>
                </span>
                <Server size={14} />
              </button>
            ))}
            {!choices.length && (
              <p className="sidebar-hint">
                没有已安装的连接插件。请先在设置 → 插件中安装或启用插件。
              </p>
            )}
          </div>
          <footer>
            <Button type="button" onClick={onClose}>
              取消
            </Button>
          </footer>
        </div>
      ) : (
        <form className="connection-form" onSubmit={save}>
          <button
            type="button"
            className="connection-back"
            onClick={() => {
              setSelected(null)
              setError('')
            }}
          >
            <ArrowLeft size={15} /> 选择其他插件
          </button>
          <p className="sidebar-hint">{current.factory.title}</p>
          <label>
            <span>连接名称</span>
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              autoFocus
              required
            />
          </label>
          {current.factory.fields.map((field) => (
            <label
              key={field.id}
              className={field.type === 'checkbox' ? 'connection-check' : undefined}
            >
              {field.type !== 'checkbox' && <span>{field.label}</span>}
              <input
                type={field.type === 'secret' ? 'password' : field.type}
                checked={field.type === 'checkbox' ? Boolean(values[field.id]) : undefined}
                value={field.type === 'checkbox' ? undefined : String(values[field.id] ?? '')}
                required={field.required}
                placeholder={field.placeholder}
                min={field.type === 'number' ? 1 : undefined}
                max={field.type === 'number' ? 65535 : undefined}
                onChange={(event) =>
                  setValues((prev) => ({
                    ...prev,
                    [field.id]:
                      field.type === 'checkbox' ? event.target.checked : event.target.value,
                  }))
                }
              />
              {field.type === 'checkbox' && <span>{field.label}</span>}
            </label>
          ))}
          {error && (
            <p className="connection-error" role="alert">
              {error}
            </p>
          )}
          <footer>
            <Button type="button" onClick={onClose}>
              取消
            </Button>
            <Button type="submit" disabled={busy || !name.trim()}>
              {busy ? '连接中…' : '添加连接'}
            </Button>
          </footer>
        </form>
      )}
    </Modal>
  )
}

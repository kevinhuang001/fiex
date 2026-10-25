import { useEffect, useMemo, useState } from 'react'
import { ArrowLeft, Cloud, HardDrive, Plug, Puzzle, Server, TerminalSquare } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { call, demo } from '@/lib/api'
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
  const choices = useMemo(() => {
    const builtin = {
      plugin: { id: '__builtin__', name: 'Fiex 内置', version: '' },
      factory: {
        id: 'ssh',
        title: 'SSH 终端',
        fields: [
          {
            id: 'host',
            label: '主机',
            type: 'text',
            required: true,
            placeholder: 'server.example.com',
          },
          { id: 'port', label: '端口', type: 'number', defaultValue: 22 },
          { id: 'username', label: '用户名', type: 'text', placeholder: 'root' },
          {
            id: 'identityFile',
            label: '私钥文件（可选）',
            type: 'text',
            placeholder: '~/.ssh/id_ed25519',
          },
          { id: 'terminalType', label: '终端类型', type: 'text', defaultValue: 'xterm-256color' },
        ] as PluginField[],
      },
    }
    const remote = plugins.flatMap((plugin) => {
      const ok = plugin.enabled && Boolean(plugin.extensions?.connections?.length)
      return ok
        ? (plugin.extensions?.connections || []).map((factory) => ({
            plugin,
            factory,
          }))
        : []
    })
    return demo ? [builtin, ...remote] : remote
  }, [plugins])
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
          typeof field.defaultValue === 'object'
            ? ''
            : (field.defaultValue ?? (field.type === 'checkbox' ? false : '')),
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
      actions={
        current ? undefined : (
          <button
            type="button"
            className="connection-more"
            onClick={() => {
              sessionStorage.setItem('fiex:open-plugin-store', 'connections')
              window.dispatchEvent(new CustomEvent('fiex:open-plugin-store'))
              onClose()
            }}
          >
            <Puzzle size={14} />
            获取更多连接插件
          </button>
        )
      }
    >
      {!current ? (
        <div className="connection-form">
          <div className="connection-plugin-picker">
            {choices.map(({ plugin, factory }) => (
              <button
                key={plugin.id + ':' + factory.id}
                type="button"
                className="connection-plugin-choice"
                onClick={() => choose(plugin.id + ':' + factory.id, factory.fields)}
              >
                {factory.id === 'ssh' ? (
                  <TerminalSquare size={20} />
                ) : factory.id === 'webdav' ? (
                  <Cloud size={20} />
                ) : factory.id === 'smb' ? (
                  <HardDrive size={20} />
                ) : (
                  <Plug size={20} />
                )}
                <span>
                  <strong>{factory.title || plugin.name}</strong>
                  <small>
                    {plugin.id === '__builtin__'
                      ? '内置 · 使用系统 OpenSSH'
                      : `${plugin.name} · ${plugin.version}`}
                  </small>
                </span>
                <Server size={14} />
              </button>
            ))}
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
            <ArrowLeft size={15} /> 选择其他类型
          </button>
          <p className="sidebar-hint">
            {current.factory.title}
            {current.plugin.id === '__builtin__' && ' · 密码/私钥口令会在终端中提示输入'}
          </p>
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

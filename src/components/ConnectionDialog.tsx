import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useState } from 'react'
import { ArrowLeft, Plug, Puzzle, Server, TerminalSquare } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { call, demo } from '@/lib/api'
import { usePluginCatalog, type PluginField } from '@/lib/plugins'
import type { RemoteConnection } from '@/lib/types'

export function ConnectionDialog({
  open,
  connection,
  onClose,
  onSaved,
}: {
  open: boolean
  connection?: RemoteConnection | null
  onClose: () => void
  onSaved: (connection: RemoteConnection) => void
}) {
  const plugins = usePluginCatalog()
  const choices = useMemo(() => {
    const builtin = {
      plugin: { id: '__builtin__', name: t('Fiex 内置'), version: '' },
      factory: {
        title: t('SSH 终端'),
        fields: [
          {
            id: 'host',
            label: t('主机'),
            type: 'text',
            required: true,
            placeholder: 'server.example.com',
          },
          { id: 'port', label: t('端口'), type: 'number', defaultValue: 22 },
          { id: 'username', label: t('用户名'), type: 'text', placeholder: 'root' },
          {
            id: 'identityFile',
            label: t('私钥文件（可选）'),
            type: 'text',
            placeholder: '~/.ssh/id_ed25519',
          },
          {
            id: 'terminalType',
            label: t('终端类型'),
            type: 'text',
            defaultValue: 'xterm-256color',
          },
        ] as PluginField[],
      },
    }
    const remote = plugins.flatMap((plugin) => {
      const factory = plugin.extensions?.connections
      return plugin.enabled && factory ? [{ plugin, factory }] : []
    })
    return demo || connection?.pluginId === '__builtin__' ? [builtin, ...remote] : remote
  }, [plugins, connection?.pluginId, getLocale()])
  const [selected, setSelected] = useState<string | null>(null)
  const [name, setName] = useState('')
  const [values, setValues] = useState<Record<string, string | boolean>>({})
  const [loading, setLoading] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const choiceKey = (pluginId: string) => pluginId
  const current = choices.find(({ plugin }) => choiceKey(plugin.id) === selected)

  useEffect(() => {
    if (!open) return
    setSelected(connection?.pluginId ?? null)
    setName(connection?.name ?? '')
    setValues({})
    setBusy(false)
    setError('')
    setLoading(Boolean(connection))
    if (!connection) return
    let active = true
    void call<{ settings: Record<string, string | number | boolean> }>('remote_connection_config', {
      id: connection.id,
    })
      .then((config) => {
        if (active)
          setValues(
            Object.fromEntries(
              Object.entries(config.settings).map(([key, value]) => [
                key,
                typeof value === 'number' ? String(value) : value,
              ]),
            ),
          )
      })
      .catch((cause) => {
        if (active) setError(String(cause))
      })
      .finally(() => {
        if (active) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [open, connection])

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
    if (!current || busy || loading) return
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
      const savedConnection = await call<RemoteConnection>('save_remote_connection', {
        input: {
          id: connection?.id,
          name,
          pluginId: current.plugin.id,
          kind:
            connection?.kind ?? (current.plugin.id === '__builtin__' ? 'ssh' : current.plugin.id),
          settings,
        },
      })
      onSaved(savedConnection)
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
      title={
        connection ? t('编辑连接') : current ? t('配置 ') + current.plugin.name : t('添加连接')
      }
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
            {t('获取更多连接插件')}
          </button>
        )
      }
    >
      {connection && !current ? (
        <p className="connection-error" role="alert">
          {t('连接插件不可用，请先启用对应插件')}
        </p>
      ) : !current ? (
        <div className="connection-form">
          <div className="connection-plugin-picker">
            {choices.map(({ plugin, factory }) => (
              <button
                key={plugin.id}
                type="button"
                className="connection-plugin-choice"
                onClick={() => choose(choiceKey(plugin.id), factory.fields)}
              >
                {plugin.id === '__builtin__' ? <TerminalSquare size={20} /> : <Plug size={20} />}
                <span>
                  <strong>{factory.title || plugin.name}</strong>
                  <small>
                    {plugin.id === '__builtin__'
                      ? t('内置 · 使用系统 OpenSSH')
                      : `${plugin.name} · ${plugin.version}`}
                  </small>
                </span>
                <Server size={14} />
              </button>
            ))}
          </div>
          <footer>
            <Button type="button" onClick={onClose}>
              {t('取消')}
            </Button>
          </footer>
        </div>
      ) : (
        <form className="connection-form" onSubmit={save}>
          {!connection && (
            <button
              type="button"
              className="connection-back"
              onClick={() => {
                setSelected(null)
                setError('')
              }}
            >
              <ArrowLeft size={15} /> {t('选择其他类型')}
            </button>
          )}
          <p className="sidebar-hint">
            {current.factory.title}
            {current.plugin.id === '__builtin__' && t(' · 密码/私钥口令会在终端中提示输入')}
          </p>
          <label>
            <span>{t('连接名称')}</span>
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
                disabled={loading || busy}
                type={field.type === 'secret' ? 'password' : field.type}
                checked={field.type === 'checkbox' ? Boolean(values[field.id]) : undefined}
                value={field.type === 'checkbox' ? undefined : String(values[field.id] ?? '')}
                required={field.required && !(connection && field.type === 'secret')}
                placeholder={
                  connection && field.type === 'secret'
                    ? t('留空保留已保存的凭据')
                    : field.placeholder
                }
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
              {t('取消')}
            </Button>
            <Button
              type="submit"
              disabled={
                busy || loading || !name.trim() || Boolean(error && !Object.keys(values).length)
              }
            >
              {loading
                ? t('加载中…')
                : busy
                  ? t('保存中…')
                  : connection
                    ? t('保存连接')
                    : t('添加连接')}
            </Button>
          </footer>
        </form>
      )}
    </Modal>
  )
}

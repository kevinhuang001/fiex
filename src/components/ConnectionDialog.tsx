import { ConnectionSetupHelp } from './connections/ConnectionSetupHelp'
import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useState } from 'react'
import { ArrowLeft, LoaderCircle, Plug, Puzzle, Server } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { call } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { openFilePicker } from '@/lib/filePicker'
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
    const remote = plugins.flatMap((plugin) => {
      const factory = plugin.extensions?.connections
      return plugin.enabled && factory ? [{ plugin, factory }] : []
    })
    return remote
  }, [plugins, connection?.pluginId, getLocale()])
  const [selected, setSelected] = useState<string | null>(null)
  const [name, setName] = useState('')
  const [values, setValues] = useState<Record<string, string | boolean>>({})
  const [loading, setLoading] = useState(false)
  const [busy, setBusy] = useState(false)
  const [configLoadFailed, setConfigLoadFailed] = useState(false)
  const choiceKey = (pluginId: string) => pluginId
  const current = choices.find(({ plugin }) => choiceKey(plugin.id) === selected)

  useEffect(() => {
    if (!open) return
    setSelected(connection?.pluginId ?? null)
    setName(connection?.name ?? '')
    setValues({})
    setBusy(false)
    setConfigLoadFailed(false)
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
        if (active) {
          setConfigLoadFailed(true)
          reportError(cause, 'connections:configuration')
        }
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
    setConfigLoadFailed(false)
  }
  const browse = async (field: PluginField, read: boolean) => {
    try {
    const picked = await openFilePicker({
      mode: 'openFile',
      extensions: field.extensions,
    })
    const path = Array.isArray(picked) ? picked[0] : picked
    if (!path) return
    if (!read) {
      setValues((prev) => ({ ...prev, [field.id]: path }))
      return
    }
    try {
      const text = await call<string>('read_preview', { path })
      setValues((prev) => ({ ...prev, [field.id]: text }))
    } catch (cause) {
      reportError(cause, 'connections:configuration')
    }
    } catch (cause) {
      reportError(cause, 'connections:file-picker')
    }
  }
  const save = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!current || busy || loading) return
    setBusy(true)
    setConfigLoadFailed(false)
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
          kind: connection?.kind ?? current.plugin.id,
          settings,
        },
      })
      onSaved(savedConnection)
    } catch (cause) {
      reportError(cause, 'connections:configuration')
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
                <Plug size={20} />
                <span>
                  <strong>{factory.title || plugin.name}</strong>
                  <small>{`${plugin.name} · ${plugin.version}`}</small>
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
                setConfigLoadFailed(false)
              }}
            >
              <ArrowLeft size={15} /> {t('选择其他类型')}
            </button>
          )}
          <ConnectionSetupHelp factory={current.factory} busy={busy} />
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
              <div
                className={field.type === 'file' || field.fromFile ? 'connection-file' : undefined}
              >
                <input
                  disabled={loading || busy}
                  type={
                    field.type === 'secret'
                      ? 'password'
                      : field.type === 'number'
                        ? 'number'
                        : field.type === 'checkbox'
                          ? 'checkbox'
                          : 'text'
                  }
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
                {(field.type === 'file' || field.fromFile) && (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={loading || busy}
                    onClick={() => void browse(field, Boolean(field.fromFile))}
                  >
                    {field.fromFile ? t('从文件读取') : t('选择文件')}
                  </Button>
                )}
              </div>
              {field.type === 'checkbox' && <span>{field.label}</span>}
            </label>
          ))}
          {busy && (
            <div className="connection-waiting" role="status" aria-live="polite">
              <LoaderCircle className="spin" size={22} />
              <strong>{t('正在验证连接')}</strong>
              <p>{t('如果浏览器已打开，请完成授权；完成后会自动继续。')}</p>
            </div>
          )}
          <footer>
            <Button type="button" onClick={onClose}>
              {t('取消')}
            </Button>
            <Button
              type="submit"
              disabled={
                busy || loading || configLoadFailed || !name.trim()
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

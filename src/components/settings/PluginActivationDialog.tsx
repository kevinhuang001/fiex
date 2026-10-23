import { useState } from 'react'
import { Check, Globe2, LoaderCircle, ShieldCheck, Terminal } from 'lucide-react'
import { call } from '@/lib/api'
import type { PluginInfo } from '@/lib/plugins'
import { activationConsent, permissionHints, permissionLabels } from '@/lib/pluginPermissions'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'

export function PluginActivationDialog({
  plugin,
  onClose,
  onActivated,
}: {
  plugin: PluginInfo
  onClose: () => void
  onActivated: () => void
}) {
  const consent = activationConsent(plugin)
  const items = [
    ...consent.permissions.map((value) => ({
      id: `permission:${value}`,
      title: permissionLabels[value] || value,
      hint: permissionHints[value],
      kind: 'permission',
    })),
    ...consent.domains.map((value) => ({
      id: `domain:${value}`,
      title: `网络目标：${value}`,
      hint: '仅允许此精确主机，不包含其他子域名。',
      kind: 'network',
    })),
    ...consent.commands.map((value) => ({
      id: `command:${value}`,
      title: `外部命令：${value}`,
      hint: '由宿主从环境变量 PATH 查找并执行。',
      kind: 'command',
    })),
  ]
  const [accepted, setAccepted] = useState<Set<string>>(new Set())
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const complete = items.every((item) => accepted.has(item.id))
  const activate = async () => {
    if (!complete || busy) return
    setBusy(true)
    setError('')
    try {
      await call('plugin_activate', { pluginId: plugin.id, consent })
      onActivated()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal
      open
      title={`激活 ${plugin.name}`}
      onClose={() => {
        if (!busy) onClose()
      }}
      className="plugin-activation-modal"
    >
      <div className="plugin-activation-intro">
        <span className="plugin-activation-emblem">
          <ShieldCheck size={24} />
        </span>
        <div>
          <strong>由你决定插件能做什么</strong>
          <p>逐项阅读并同意全部申请后，插件才会激活。取消不会授予任何新权限。</p>
        </div>
      </div>
      <div className="plugin-review-progress">
        <span>权限审核</span>
        <strong>
          {items.filter((item) => accepted.has(item.id)).length} / {items.length}
        </strong>
      </div>
      <div className="plugin-review-items">
        {items.map((item) => (
          <label
            key={item.id}
            className={`plugin-review-item ${accepted.has(item.id) ? 'accepted' : ''}`}
          >
            <span className="plugin-review-icon">
              {item.kind === 'command' ? (
                <Terminal size={18} />
              ) : item.kind === 'network' ? (
                <Globe2 size={18} />
              ) : (
                <ShieldCheck size={18} />
              )}
            </span>
            <span className="plugin-review-copy">
              <strong>{item.title}</strong>
              <small>{item.hint}</small>
            </span>
            <input
              type="checkbox"
              aria-label={`同意${item.title}`}
              checked={accepted.has(item.id)}
              disabled={busy}
              onChange={(event) =>
                setAccepted((old) => {
                  const next = new Set(old)
                  if (event.target.checked) next.add(item.id)
                  else next.delete(item.id)
                  return next
                })
              }
            />
          </label>
        ))}
        {!items.length && <p className="sidebar-hint">此插件没有申请受控资源权限。</p>}
      </div>
      {error && (
        <p role="alert" className="plugin-page-message">
          {error}
        </p>
      )}
      <footer className="plugin-consent-actions">
        <Button variant="outline" disabled={busy} onClick={onClose}>
          取消
        </Button>
        <Button disabled={!complete || busy} onClick={() => void activate()}>
          {busy ? <LoaderCircle size={15} className="spin" /> : <Check size={15} />}同意并激活
        </Button>
      </footer>
    </Modal>
  )
}

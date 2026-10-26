import { useState } from 'react'
import { Check, LoaderCircle } from 'lucide-react'
import { call } from '@/lib/api'
import type { PluginInfo } from '@/lib/plugins'
import {
  activationConsent,
  activationGroups,
  type ActivationConsent,
} from '@/lib/pluginPermissions'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import { PluginGlyph } from '@/components/plugins/PluginGlyph'

/**
 * One consent sheet for the whole plugin: files and folders together, network
 * targets, commands and other capabilities. Every declared permission, domain
 * and command has its own toggle and everything starts selected. Activation
 * submits only the still-selected subset, so deselecting is a complete answer.
 */
export function PluginActivationDialog({
  plugin,
  onClose,
  onActivated,
}: {
  plugin: PluginInfo
  onClose: () => void
  onActivated: () => void
}) {
  const [groups] = useState(() => activationGroups(plugin))
  const [selected, setSelected] = useState<ActivationConsent>(() => activationConsent(plugin))
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  const togglePermission = (permission: string, enabled: boolean) => {
    setSelected((current) => ({
      ...current,
      permissions: enabled
        ? [...new Set([...current.permissions, permission])].sort()
        : current.permissions.filter((value) => value !== permission),
      // A target without its capability is meaningless, so switching the
      // capability off also clears the targets it could reach.
      networkAll: permission === 'network' && !enabled ? false : current.networkAll,
      domains: permission === 'network' && !enabled ? [] : current.domains,
      commands: permission === 'commands' && !enabled ? [] : current.commands,
    }))
  }

  const toggleTarget = (kind: 'network' | 'commands', value: string, enabled: boolean) => {
    setSelected((current) => {
      const key = kind === 'network' ? 'domains' : 'commands'
      const values = enabled
        ? [...new Set([...current[key], value])].sort()
        : current[key].filter((item) => item !== value)
      // Selecting a target implies the capability; this keeps the submitted
      // subset consistent when the parent toggle was switched off first.
      const permissions =
        enabled && !current.permissions.includes(kind)
          ? [...current.permissions, kind].sort()
          : current.permissions
      return { ...current, permissions, [key]: values }
    })
  }

  const toggleNetworkAll = (enabled: boolean) => {
    setSelected((current) => ({
      ...current,
      networkAll: enabled,
      permissions:
        enabled && !current.permissions.includes('network')
          ? [...current.permissions, 'network'].sort()
          : current.permissions,
    }))
  }

  const activate = async () => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      await call('plugin_activate', { pluginId: plugin.id, consent: selected })
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
      title="激活插件"
      className="plugin-activation-modal"
      onClose={() => {
        if (!busy) onClose()
      }}
    >
      <div className="plugin-activation-page">
        <div className="plugin-activation-head">
          <span className="plugin-activation-icon">
            <PluginGlyph icon={plugin.icon} size={32} />
          </span>
          <div className="plugin-activation-head-copy">
            <strong>{plugin.name}</strong>
            <p>
              选择要授予的能力与目标。未选择的能力不会授权；插件需要时仍会单独询问，之后也可在插件详情中调整。
            </p>
          </div>
        </div>

        {groups.map((group) => (
          <section
            className="plugin-activation-section"
            key={group.id}
            aria-labelledby={`plugin-activation-section-${group.id}`}
          >
            <div className="plugin-activation-section-head">
              <h3 id={`plugin-activation-section-${group.id}`}>{group.title}</h3>
              <p>{group.hint}</p>
            </div>
            <div className="plugin-activation-rows">
              {group.permissions.map((permission) => {
                const hintId = `plugin-activation-hint-${group.id}-${permission.id}`
                return (
                  <label className="plugin-activation-row" key={permission.id}>
                    <span className="plugin-activation-row-copy">
                      <strong>{permission.label}</strong>
                      {permission.hint && <small id={hintId}>{permission.hint}</small>}
                    </span>
                    <input
                      type="checkbox"
                      aria-label={permission.label}
                      aria-describedby={permission.hint ? hintId : undefined}
                      checked={selected.permissions.includes(permission.id)}
                      disabled={busy}
                      onChange={(event) => togglePermission(permission.id, event.target.checked)}
                    />
                  </label>
                )
              })}
              {group.networkAll && (
                <label className="plugin-activation-row danger">
                  <span className="plugin-activation-row-copy">
                    <strong>访问全部网络</strong>
                    <small
                      className="plugin-activation-warning"
                      id="plugin-activation-network-all"
                      role="alert"
                    >
                      危险：插件将可以连接任意主机，而不只是清单中列出的域名。只在你完全信任该插件时选择。
                    </small>
                  </span>
                  <input
                    type="checkbox"
                    aria-label="访问全部网络"
                    aria-describedby="plugin-activation-network-all"
                    checked={selected.networkAll}
                    disabled={busy}
                    onChange={(event) => toggleNetworkAll(event.target.checked)}
                  />
                </label>
              )}
              {group.domains.map((domain) => {
                const hintId = `plugin-activation-domain-${domain}`
                return (
                  <label className="plugin-activation-row" key={`domain:${domain}`}>
                    <span className="plugin-activation-row-copy">
                      <strong>{domain}</strong>
                      <small id={hintId}>插件申请的精确域名，不包含其他子域名。</small>
                    </span>
                    <input
                      type="checkbox"
                      aria-label={domain}
                      aria-describedby={hintId}
                      checked={selected.domains.includes(domain)}
                      disabled={busy}
                      onChange={(event) => toggleTarget('network', domain, event.target.checked)}
                    />
                  </label>
                )
              })}
              {group.commands.map((command) => {
                const hintId = `plugin-activation-command-${command.name}`
                return (
                  <label className="plugin-activation-row" key={`command:${command.name}`}>
                    <span className="plugin-activation-row-copy">
                      <strong>
                        <code>{command.name}</code>
                      </strong>
                      <small
                        id={hintId}
                        className={command.missing ? 'plugin-activation-warning' : undefined}
                      >
                        {command.missing
                          ? '本地未找到该命令，允许后插件也可能无法工作。'
                          : '仅允许此精确命令名，运行时以当前用户身份执行。'}
                      </small>
                    </span>
                    <input
                      type="checkbox"
                      aria-label={command.name}
                      aria-describedby={hintId}
                      checked={selected.commands.includes(command.name)}
                      disabled={busy}
                      onChange={(event) =>
                        toggleTarget('commands', command.name, event.target.checked)
                      }
                    />
                  </label>
                )
              })}
            </div>
          </section>
        ))}

        {!groups.length && (
          <p className="plugin-activation-empty">此插件未申请受控资源，激活后即可运行。</p>
        )}
        {error && (
          <p role="alert" className="plugin-activation-error">
            {error}
          </p>
        )}

        <div className="plugin-activation-actions">
          <Button
            type="button"
            variant="outline"
            disabled={busy}
            onClick={() => {
              if (!busy) onClose()
            }}
          >
            取消
          </Button>
          <Button type="button" disabled={busy} onClick={() => void activate()}>
            {busy ? <LoaderCircle className="spin" size={15} /> : <Check size={15} />}
            {busy ? '正在激活…' : '激活'}
          </Button>
        </div>
      </div>
    </Modal>
  )
}

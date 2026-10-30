import { t } from '@/i18n'
import { useState } from 'react'
import { call } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import type { PluginInfo } from '@/lib/plugins'
import {
  activationConsent,
  activationGroups,
  type ActivationConsent,
  type ActivationGroup,
} from '@/lib/pluginPermissions'
import { PluginPermissionPrompt } from './PluginPermissionPrompt'

/**
 * Grouped activation: one mobile-style prompt per permission group (files and
 * folders, network, commands, other). Each prompt contains individual toggles
 * for its capabilities/targets; the last prompt activates only the selected
 * subset, so cancelling any group grants nothing.
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
  const [index, setIndex] = useState(0)
  const [selected, setSelected] = useState<ActivationConsent>(() => activationConsent(plugin))
  const [busy, setBusy] = useState(false)
  const group = groups[Math.min(index, Math.max(0, groups.length - 1))]
  const last = index + 1 >= groups.length

  const togglePermission = (permission: string, enabled: boolean) => {
    setSelected((current) => ({
      ...current,
      permissions: enabled
        ? [...new Set([...current.permissions, permission])].sort()
        : current.permissions.filter((value) => value !== permission),
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
    try {
      await call('plugin_activate', { pluginId: plugin.id, consent: selected })
      onActivated()
    } catch (cause) {
      reportError(cause, 'plugins:activation')
    } finally {
      setBusy(false)
    }
  }

  const next = () => {
    if (busy) return
    if (last) void activate()
    else setIndex(index + 1)
  }

  const item = group
    ? { id: `group:${group.id}`, kind: 'permission' as const, title: group.title }
    : {
        id: 'activate',
        kind: 'activate' as const,
        title: t('运行插件'),
      }

  return (
    <PluginPermissionPrompt
      open
      pluginName={plugin.name}
      icon={plugin.icon}
      item={item}
      step={index + 1}
      total={Math.max(1, groups.length)}
      busy={busy}
      allowLabel={last ? t('激活') : t('继续')}
      denyLabel={t('取消')}
      onAllow={next}
      onDeny={() => {
        if (!busy) onClose()
      }}
    >
      {group && (
        <div className="plugin-permission-prompt-toggles" role="group" aria-label={group.title}>
          <GroupToggles
            group={group}
            selected={selected}
            busy={busy}
            onPermission={togglePermission}
            onTarget={toggleTarget}
            onNetworkAll={toggleNetworkAll}
          />
        </div>
      )}
    </PluginPermissionPrompt>
  )
}

function GroupToggles({
  group,
  selected,
  busy,
  onPermission,
  onTarget,
  onNetworkAll,
}: {
  group: ActivationGroup
  selected: ActivationConsent
  busy: boolean
  onPermission: (permission: string, enabled: boolean) => void
  onTarget: (kind: 'network' | 'commands', value: string, enabled: boolean) => void
  onNetworkAll: (enabled: boolean) => void
}) {
  return (
    <>
      {group.permissions.map((permission) => (
        <label className="plugin-permission-prompt-toggle" key={permission.id}>
          <span>
            <strong>{permission.label}</strong>
            {permission.hint && <small>{permission.hint}</small>}
          </span>
          <input
            type="checkbox"
            aria-label={permission.label}
            checked={selected.permissions.includes(permission.id)}
            disabled={busy}
            onChange={(event) => onPermission(permission.id, event.target.checked)}
          />
        </label>
      ))}
      {group.networkAll && (
        <label className="plugin-permission-prompt-toggle danger">
          <span>
            <strong>{t('访问全部网络')}</strong>
            <small role="alert">{t('危险：允许连接任意主机，而不只是清单中的域名。')}</small>
          </span>
          <input
            type="checkbox"
            aria-label={t('访问全部网络')}
            checked={selected.networkAll}
            disabled={busy}
            onChange={(event) => onNetworkAll(event.target.checked)}
          />
        </label>
      )}
      {group.domains.map((domain) => (
        <label className="plugin-permission-prompt-toggle" key={`domain:${domain}`}>
          <span>
            <strong>{domain}</strong>
            <small>{t('仅授权此精确主机，不包含其他子域名。')}</small>
          </span>
          <input
            type="checkbox"
            aria-label={domain}
            checked={selected.domains.includes(domain)}
            disabled={busy}
            onChange={(event) => onTarget('network', domain, event.target.checked)}
          />
        </label>
      ))}
      {group.commands.map((command) => (
        <label className="plugin-permission-prompt-toggle" key={`command:${command.name}`}>
          <span>
            <strong>
              <code>{command.name}</code>
            </strong>
            <small className={command.missing ? 'plugin-permission-prompt-warning' : undefined}>
              {command.missing
                ? t('本地未找到该命令，允许后插件也可能无法工作。')
                : t('仅允许此精确命令名。')}
            </small>
          </span>
          <input
            type="checkbox"
            aria-label={command.name}
            checked={selected.commands.includes(command.name)}
            disabled={busy}
            onChange={(event) => onTarget('commands', command.name, event.target.checked)}
          />
        </label>
      ))}
    </>
  )
}

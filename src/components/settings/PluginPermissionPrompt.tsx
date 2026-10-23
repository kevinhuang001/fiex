import { Check, LoaderCircle } from 'lucide-react'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import { PluginGlyph } from '@/components/plugins/PluginGlyph'
import type { PluginInfo } from '@/lib/plugins'
import {
  activationConsent,
  commandRisk,
  permissionHints,
  permissionLabels,
} from '@/lib/pluginPermissions'

export interface PluginPermissionItem {
  id: string
  kind: 'permission' | 'network' | 'command' | 'activate'
  title: string
  hint?: string
  value?: string
  /** Shown in red, e.g. a command that is not installed on this machine. */
  warning?: string
}

const permissionOrder = [
  'folders.read',
  'folders.create',
  'folders.delete',
  'folders.rename',
  'files.read',
  'files.write',
  'network',
  'commands',
]
const rank = (value: string) => {
  const index = permissionOrder.indexOf(value)
  return index === -1 ? permissionOrder.length : index
}

/** Every declared capability, network host and command becomes its own
 * mobile-style request. The order is stable so the review is predictable. */
export function activationPromptItems(plugin: PluginInfo): PluginPermissionItem[] {
  const consent = activationConsent(plugin)
  const items = [
    ...[...consent.permissions]
      .sort((a, b) => rank(a) - rank(b))
      .map((value) => ({
        id: `permission:${value}`,
        kind: 'permission' as const,
        title: permissionLabels[value] || value,
        hint: permissionHints[value],
      })),
    ...consent.domains.map((value) => ({
      id: `domain:${value}`,
      kind: 'network' as const,
      title: '网络域名',
      value,
    })),
    ...consent.commands.map((value) => ({
      id: `command:${value}`,
      kind: 'command' as const,
      title: '外部命令',
      value,
      warning: plugin.missingCommands?.includes(value)
        ? '本地未找到该命令，允许后插件也可能无法工作。'
        : undefined,
    })),
  ]
  return items.length
    ? items
    : [
        {
          id: 'activate',
          kind: 'activate' as const,
          title: '运行',
          hint: '此插件未申请受控资源。',
        },
      ]
}

/** Request order for a single permission that was switched on again. */
export function promptForPermission(permission: string): PluginPermissionItem {
  return {
    id: `permission:${permission}`,
    kind: 'permission',
    title: permissionLabels[permission] || permission,
    hint: permissionHints[permission],
  }
}

/**
 * Mobile-style permission prompt: one request at a time, with a single
 * allow/deny decision. Activation only calls Core after the last request is
 * allowed, so denying any request grants nothing.
 */
export function PluginPermissionPrompt({
  open,
  pluginName,
  icon,
  item,
  step,
  total,
  busy,
  error,
  allowLabel = '允许',
  denyLabel = '不允许',
  onAllow,
  onDeny,
}: {
  open: boolean
  pluginName: string
  icon?: string
  item: PluginPermissionItem
  step: number
  total: number
  busy?: boolean
  error?: string
  allowLabel?: string
  denyLabel?: string
  onAllow: () => void
  onDeny: () => void
}) {
  const question =
    item.kind === 'network'
      ? `允许「${pluginName}」访问 ${item.value}？`
      : item.kind === 'command'
        ? `允许「${pluginName}」执行 ${item.value}？`
        : item.kind === 'activate'
          ? `允许「${pluginName}」运行？`
          : `允许「${pluginName}」${item.title}？`
  const hint =
    item.kind === 'network'
      ? '仅授权此精确主机，不包含其他子域名；插件本身不能直接联网。'
      : item.kind === 'command'
        ? commandRisk
        : item.hint
  return (
    <Modal
      open={open}
      title="权限请求"
      onClose={onDeny}
      className="plugin-permission-prompt-modal"
      bare
    >
      <div className="plugin-permission-prompt" role="group" aria-label={question}>
        <span className="plugin-permission-prompt-app">
          <PluginGlyph icon={icon} size={34} />
        </span>
        <span className="plugin-permission-prompt-app-name">{pluginName}</span>
        {total > 1 && (
          <span className="plugin-permission-prompt-step">
            第 {step} 项，共 {total} 项
          </span>
        )}
        <h3>{question}</h3>
        {hint && <p>{hint}</p>}
        {item.warning && (
          <p role="alert" className="plugin-permission-prompt-warning">
            {item.warning}
          </p>
        )}
        {error && (
          <p role="alert" className="plugin-permission-prompt-error">
            {error}
          </p>
        )}
        <div className="plugin-permission-prompt-actions">
          <Button type="button" variant="outline" disabled={busy} onClick={onDeny}>
            {denyLabel}
          </Button>
          <Button type="button" disabled={busy} onClick={onAllow}>
            {busy ? <LoaderCircle className="spin" size={15} /> : <Check size={15} />}
            {busy ? '正在激活…' : allowLabel}
          </Button>
        </div>
      </div>
    </Modal>
  )
}

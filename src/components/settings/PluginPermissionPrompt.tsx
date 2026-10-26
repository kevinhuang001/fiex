import { Check, LoaderCircle } from 'lucide-react'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import { PluginGlyph } from '@/components/plugins/PluginGlyph'
import { commandRisk, permissionHints, permissionLabels } from '@/lib/pluginPermissions'

export interface PluginPermissionItem {
  id: string
  kind: 'permission' | 'network' | 'networkAll' | 'command' | 'activate'
  title: string
  hint?: string
  value?: string
  /** Shown in red, e.g. a command that is not installed on this machine. */
  warning?: string
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
  note,
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
  /** Extra explanation for operation-time requests. */
  note?: string
  allowLabel?: string
  denyLabel?: string
  onAllow: () => void
  onDeny: () => void
}) {
  const question =
    item.kind === 'networkAll'
      ? `允许「${pluginName}」访问全部网络？`
      : item.kind === 'network'
        ? `允许「${pluginName}」访问 ${item.value}？`
        : item.kind === 'command'
          ? `允许「${pluginName}」执行 ${item.value}？`
          : item.kind === 'activate'
            ? `允许「${pluginName}」运行？`
            : `允许「${pluginName}」${item.title}？`
  const hint =
    item.kind === 'networkAll'
      ? '这不是精确域名授权：插件可以访问任意 HTTP/TCP 目标。建议先改用白名单。'
      : item.kind === 'network'
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
      <div
        className={`plugin-permission-prompt ${item.kind === 'networkAll' ? 'danger' : ''}`}
        role="group"
        aria-label={question}
      >
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
        {note && <p className="plugin-permission-prompt-note">{note}</p>}
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

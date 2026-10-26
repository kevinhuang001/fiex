import { t } from '@/i18n'
import { Check, LoaderCircle } from 'lucide-react'
import type { ReactNode } from 'react'
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
  allowLabel = t('允许'),
  denyLabel = t('不允许'),
  stepUnit = t('项'),
  children,
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
  /** Unit for the step counter, e.g. groups during activation. */
  stepUnit?: string
  /** Extra controls (toggles) rendered above the actions. */
  children?: ReactNode
  onAllow: () => void
  onDeny: () => void
}) {
  const question =
    item.kind === 'networkAll'
      ? t('允许「{0}」访问全部网络？', pluginName)
      : item.kind === 'network'
        ? t('允许「{0}」访问 {1}？', pluginName, item.value)
        : item.kind === 'command'
          ? t('允许「{0}」执行 {1}？', pluginName, item.value)
          : item.kind === 'activate'
            ? t('允许「{0}」运行？', pluginName)
            : t('允许「{0}」{1}？', pluginName, item.title)
  const hint =
    item.kind === 'networkAll'
      ? t('这不是精确域名授权：插件可以访问任意 HTTP/TCP 目标。建议先改用白名单。')
      : item.kind === 'network'
        ? t('仅授权此精确主机，不包含其他子域名；插件本身不能直接联网。')
        : item.kind === 'command'
          ? t(commandRisk)
          : item.hint
  return (
    <Modal
      open={open}
      title={t('权限请求')}
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
            {t('第')}
            {step} {stepUnit}
            {t('，共')}
            {total} {stepUnit}
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
        {children}
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
            {busy ? t('正在激活…') : allowLabel}
          </Button>
        </div>
      </div>
    </Modal>
  )
}

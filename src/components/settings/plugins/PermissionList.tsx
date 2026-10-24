import { Globe2, ShieldCheck, Terminal } from 'lucide-react'
import type { PluginInfo } from '@/lib/plugins'
import { permissionHints, permissionLabels } from '@/lib/pluginPermissions'

export function PermissionList({
  plugin,
  busy,
  onToggle,
  onConfigure,
}: {
  plugin: PluginInfo
  busy: boolean
  onToggle: (permission: string, granted: boolean) => void
  onConfigure: (permission: string) => void
}) {
  const declared = plugin.declaredPermissions || []
  const granted = plugin.grantedPermissions || []
  const configurable = (permission: string) => permission === 'network' || permission === 'commands'
  return (
    <>
      {declared.map((permission) => (
        <div className="plugin-permission-card" key={permission}>
          <span className="plugin-permission-icon">
            <ShieldCheck size={15} />
          </span>
          <span className="plugin-permission-copy">
            <strong>{permissionLabels[permission] || permission}</strong>
            <small>{permissionHints[permission] || permission}</small>
            {permission === 'commands' && Boolean(plugin.missingCommands?.length) && (
              <small className="plugin-command-missing">
                本地未找到：{(plugin.missingCommands || []).join('、')}，插件可能无法正常工作
              </small>
            )}
          </span>
          {configurable(permission) && (
            <button
              type="button"
              className="plugin-inline-config"
              aria-label={permission === 'network' ? '配置网络域名' : '配置允许的命令'}
              title={permission === 'network' ? '配置网络域名' : '配置允许的命令'}
              disabled={busy || !plugin.approved || !granted.includes(permission)}
              onClick={() => onConfigure(permission)}
            >
              {permission === 'network' ? <Globe2 size={13} /> : <Terminal size={13} />}
            </button>
          )}
          <button
            type="button"
            role="switch"
            aria-checked={granted.includes(permission)}
            aria-label={`授权 ${permissionLabels[permission] || permission}`}
            className={`plugin-switch ${granted.includes(permission) ? 'on' : ''}`}
            disabled={busy || !plugin.approved}
            onClick={() => onToggle(permission, !granted.includes(permission))}
          >
            <span />
          </button>
        </div>
      ))}
      {!declared.length && <p className="sidebar-hint">此插件未声明受控宿主权限。</p>}
    </>
  )
}

/** Settings stored before `list` existed may be a comma/newline separated
 * string; accept both shapes and always hand the editor a string array. */

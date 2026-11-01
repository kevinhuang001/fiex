import { t } from '@/i18n'
import { ShieldCheck } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import type { PluginInfo } from '@/lib/plugins'
import { PermissionList } from './PermissionList'
import { pluginIcon } from '../PluginInstaller'

/**
 * Plugin detail: identity, consent state and the per-permission switches.
 * Permission writes stay in the page and arrive as callbacks.
 */
export function PluginPermissionsSheet({
  plugin,
  busy,
  onClose,
  onActivate,
  onTogglePermission,
  onConfigure,
}: {
  plugin: PluginInfo | null
  busy: boolean
  onClose: () => void
  onActivate: (plugin: PluginInfo) => void
  onTogglePermission: (plugin: PluginInfo, permission: string, granted: boolean) => void
  onConfigure: (permission: string) => void
}) {
  return (
    <Modal
      open={Boolean(plugin)}
      onClose={onClose}
      title={t('插件详情')}
      className="plugin-permissions-modal"
    >
      {plugin && (
        <div className="plugin-permissions-page">
          <div className="plugin-permission-profile">
            <div className={`plugin-permission-profile-icon ${plugin.enabled ? 'enabled' : ''}`}>
              {pluginIcon(plugin.icon)}
            </div>
            <div className="plugin-permission-profile-copy">
              <div className="plugin-permission-profile-title">
                <strong>{plugin.name}</strong>
                <span className={`plugin-state ${plugin.enabled ? 'active' : ''}`}>
                  <span />
                  {plugin.enabled ? t('运行中') : t('未激活')}
                </span>
                {!plugin.approved && <span className="plugin-pill warn">{t('待授权')}</span>}
              </div>
              <small>
                {plugin.id} · v{plugin.version}
              </small>
              <p>{plugin.description || t('没有提供说明。')}</p>
            </div>
          </div>
          {!plugin.approved && (
            <div className="plugin-consent-banner">
              <ShieldCheck size={18} />
              <div>
                <strong>{t('此插件尚未授权')}</strong>
                <p>{t('激活时可以逐项选择权限，未选择的能力不会授权。')}</p>
              </div>
              <Button type="button" size="sm" onClick={() => onActivate(plugin)}>
                {t('审核并激活')}
              </Button>
            </div>
          )}
          <section className="plugin-permission-section">
            <div className="plugin-section-title">
              <strong>
                <ShieldCheck size={15} /> {t('权限')}
              </strong>
              <small>
                {(plugin.grantedPermissions || []).length} /{' '}
                {(plugin.declaredPermissions || []).length} {t('已授权')}
              </small>
            </div>
            <div className="plugin-permission-list">
              <PermissionList
                plugin={plugin}
                busy={busy}
                onToggle={(permission, granted) => onTogglePermission(plugin, permission, granted)}
                onConfigure={onConfigure}
              />
            </div>
          </section>
        </div>
      )}
    </Modal>
  )
}

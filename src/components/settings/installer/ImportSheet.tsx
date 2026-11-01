import { t } from '@/i18n'
import { FileText, FolderOpen, Globe2, Info, ShieldCheck, Terminal, Zap } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import { permissionHints, permissionLabels } from '@/lib/pluginPermissions'
import { pluginIcon } from './pluginIcon'
import type { InstallPreview } from './types'

const permissionIcons: Record<string, typeof Info> = {
  metadata: Info,
  'folders.read': FolderOpen,
  'folders.create': FolderOpen,
  'folders.delete': FolderOpen,
  'folders.rename': FolderOpen,
  'files.read': FileText,
  'files.write': FileText,
  'files.create': FileText,
  'files.delete': FileText,
  'files.rename': FileText,
  network: Globe2,
  commands: Terminal,
  hooks: Zap,
}

/** The install consent sheet: an introduction and the permissions the package
 *  actually asks for. Trust details stay in the plugin details sheet so this
 *  decision stays short and readable. */
export function ImportSheet({
  preview,
  installing,
  onDiscard,
  onConfirm,
}: {
  preview: InstallPreview | null
  installing: boolean
  onDiscard: () => void
  onConfirm: () => void
}) {
  const permissions = preview?.permissions || []
  return (
    <Modal
      open={Boolean(preview)}
      title={preview?.isUpdate ? t('更新插件') : t('安装插件')}
      onClose={onDiscard}
      className="plugin-consent-modal"
    >
      <div className="plugin-consent">
        <div className="plugin-consent-head">
          <div className="plugin-consent-icon">{pluginIcon(preview?.icon)}</div>
          <div className="plugin-consent-identity">
            <div className="plugin-consent-name">
              <strong>{preview?.name}</strong>
              <span>
                {preview?.isUpdate ? (
                  <>
                    {preview?.previousVersion} → {preview?.version}
                  </>
                ) : (
                  <>v{preview?.version}</>
                )}
              </span>
            </div>
            <p>
              {t('作者：')}
              {preview?.author?.name || t('未知开发者')}
            </p>
          </div>
        </div>

        <p className="plugin-consent-description">{preview?.description || t('没有提供说明。')}</p>

        <section className="plugin-consent-section">
          <h3>{t('请求的权限')}</h3>
          <div className="plugin-consent-permissions">
            {permissions.map((permission) => {
              const Icon = permissionIcons[permission] || ShieldCheck
              return (
                <span
                  className="plugin-consent-chip"
                  key={permission}
                  title={permissionHints[permission] || permissionLabels[permission] || permission}
                >
                  <Icon size={13} />
                  {permissionLabels[permission] || permission}
                </span>
              )
            })}
            {preview?.requestedNetworkAll && (
              <span className="plugin-consent-chip danger">
                <Globe2 size={13} />
                {t('全部网络')}
              </span>
            )}
            {!permissions.length && !preview?.requestedNetworkAll && (
              <span className="plugin-consent-empty">
                <ShieldCheck size={14} /> {t('无需宿主权限')}
              </span>
            )}
          </div>
          {!!preview?.requestedNetworkDomains.length && (
            <p className="plugin-consent-detail">
              {t('网络域名')}：
              {preview.requestedNetworkDomains.map((domain) => (
                <code key={domain}>{domain}</code>
              ))}
            </p>
          )}
          {!!preview?.commands?.length && (
            <p className="plugin-consent-detail">
              {t('外部命令')}：
              {preview.commands.map((command) => (
                <code key={command}>{command}</code>
              ))}
            </p>
          )}
        </section>

        <footer className="plugin-consent-actions">
          <Button variant="outline" type="button" disabled={installing} onClick={onDiscard}>
            {t('取消')}
          </Button>
          <Button type="button" disabled={installing} onClick={() => void onConfirm()}>
            {installing
              ? preview?.isUpdate
                ? t('更新中…')
                : t('安装中…')
              : preview?.isUpdate
                ? t('更新并授权激活')
                : t('安装并授权激活')}
          </Button>
        </footer>
      </div>
    </Modal>
  )
}

import { Shield } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import { permissionLabels as permissionTitles } from '@/lib/pluginPermissions'
import { TrustNotice } from './TrustNotice'
import type { InstallPreview } from './types'

/** The consent sheet: exact permissions, requested commands, network domains
 *  and the trust state, confirmed in one step. */
export function ImportSheet({
  preview,
  error,
  installing,
  onDiscard,
  onConfirm,
}: {
  preview: InstallPreview | null
  error: string
  installing: boolean
  onDiscard: () => void
  onConfirm: () => void
}) {
  return (
    <Modal
      open={Boolean(preview)}
      title={preview?.isUpdate ? '确认插件更新及权限' : '确认插件安装及权限'}
      onClose={onDiscard}
    >
      <div className="plugin-install-review">
        <div className="plugin-install-review-heading">
          <Shield size={21} />
          <div>
            <strong>{preview?.name}</strong>
            <small>
              {preview?.id} · {preview?.isUpdate ? `${preview?.previousVersion} → ` : ''}
              {preview?.version}
            </small>
          </div>
        </div>
        <div className="plugin-install-meta">
          <span>作者：{preview?.author?.name || '未知开发者'}</span>
          <span>来源：GitHub / {preview?.repository}</span>
        </div>
        <TrustNotice preview={preview} />
        <strong>请求的权限</strong>
        <div className="plugin-install-permissions">
          {preview?.permissions.length ? (
            preview.permissions.map((permission) => (
              <div key={permission}>
                <Shield size={14} />
                {permissionTitles[permission] || permission}
              </div>
            ))
          ) : (
            <span>无需宿主权限</span>
          )}
        </div>
        {!!preview?.commands?.length && (
          <>
            <strong>插件申请的命令</strong>
            <div className="plugin-install-domains">
              {(preview.commands || []).map((command) => (
                <span key={command}>
                  <code>{command}</code>
                </span>
              ))}
            </div>
            <p className="sidebar-hint">
              命令由宿主代理执行，插件进程不能直接启动程序；安装后可在设置里增删。
            </p>
          </>
        )}
        {!!preview?.requestedNetworkDomains.length && (
          <>
            <strong>插件申请的网络域名</strong>
            <div className="plugin-install-domains">
              {preview.requestedNetworkDomains.map((domain) => (
                <span key={domain}>{domain}</span>
              ))}
            </div>
            <p className="sidebar-hint">安装确认会同时授权这些精确域名，之后可以增删。</p>
          </>
        )}
        <p className="sidebar-hint">
          所有插件（包括 Fiex 官方发布的插件）使用同样的授权流程。插件为 Native
          程序；在系统沙箱完善前，权限控制仅约束宿主代理的操作，不能阻止插件直接进行系统调用。
        </p>
        {error && (
          <p role="alert" className="connection-error">
            {error}
          </p>
        )}
        <footer className="plugin-consent-actions">
          <Button variant="outline" type="button" disabled={installing} onClick={onDiscard}>
            取消
          </Button>
          <Button type="button" disabled={installing} onClick={() => void onConfirm()}>
            {installing
              ? preview?.isUpdate
                ? '更新中…'
                : '安装中…'
              : preview?.isUpdate
                ? '更新（权限变更需重新激活）'
                : '安装（权限默认关闭）'}
          </Button>
        </footer>
      </div>
    </Modal>
  )
}

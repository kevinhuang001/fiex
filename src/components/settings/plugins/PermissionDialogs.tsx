import { t } from '@/i18n'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import type { PluginInfo } from '@/lib/plugins'

function networkDomains(plugin: PluginInfo) {
  return [
    ...new Set([
      ...(plugin.requestedNetworkDomains || []),
      ...(plugin.allowedNetworkDomains || []),
    ]),
  ].sort()
}

function pluginCommands(plugin: PluginInfo) {
  return [
    ...new Set([...(plugin.requestedCommands || []), ...(plugin.allowedCommands || [])]),
  ].sort()
}

/** Exact hosts the plugin may reach. Localhost and LAN hosts stay explicit. */
export function DomainDialog({
  plugin,
  busy,
  draft,
  onDraftChange,
  onToggle,
  onAdd,
  onToggleAll,
  onClose,
}: {
  plugin: PluginInfo
  busy: boolean
  draft: string
  onDraftChange: (value: string) => void
  onToggle: (domain: string, granted: boolean) => void
  onAdd: (domain: string) => void
  onToggleAll: (all: boolean) => void
  onClose: () => void
}) {
  const granted = plugin.grantedPermissions.includes('network')
  const domains = networkDomains(plugin)
  const add = () => {
    const domain = draft.trim()
    if (domain) onAdd(domain)
  }
  const all = plugin.allowedNetworkAll
  return (
    <Modal open onClose={onClose} title={t('网络域名')} className="plugin-domain-modal">
      <div className="plugin-domain-page">
        <p className="plugin-domain-hint">
          {t('允许“{0}”访问的精确主机；本机和局域网地址也需要授权。', plugin.name)}
        </p>
        <label className="plugin-network-mode">
          <span>{t('网络范围')}</span>
          <select
            aria-label={t('网络访问范围')}
            value={all ? 'all' : 'whitelist'}
            onChange={(event) => onToggleAll(event.target.value === 'all')}
          >
            <option value="whitelist">{t('白名单')}</option>
            <option value="all">{t('全部域名')}</option>
          </select>
        </label>
        {all ? (
          <p className="plugin-network-danger" role="alert">
            {t('危险：插件当前可以访问任意主机。切换为白名单后只添加需要的域名。')}
          </p>
        ) : (
          ''
        )}
        {!all && plugin.requestedNetworkAll && (
          <p className="plugin-network-request" role="status">
            {t('插件申请了全部网络访问；你仍可以使用白名单限制它。')}
          </p>
        )}
        {!all && (
          <section className="plugin-permission-section plugin-domain-section">
            <div className="plugin-domain-list">
              {domains.map((domain) => {
                const allowed = (plugin.allowedNetworkDomains || []).includes(domain)
                const requested = (plugin.requestedNetworkDomains || []).includes(domain)
                return (
                  <div className="plugin-domain-row" key={domain}>
                    <span>
                      {domain}
                      {requested && <small>{t('插件申请')}</small>}
                      {!allowed && <small className="muted">{t('未授权')}</small>}
                    </span>
                    <button
                      type="button"
                      role="switch"
                      aria-checked={allowed}
                      aria-label={allowed ? t('移除域名 {0}', domain) : t('授权域名 {0}', domain)}
                      title={allowed ? t('移除域名') : t('授权域名')}
                      className={`plugin-switch ${allowed ? 'on' : ''}`}
                      disabled={busy || !granted}
                      onClick={() => onToggle(domain, !allowed)}
                    >
                      <span />
                    </button>
                  </div>
                )
              })}
              {!domains.length && <p className="sidebar-hint">{t('尚未允许任何域名。')}</p>}
            </div>
            <div className="plugin-domain-add">
              <input
                aria-label={t('添加允许的域名')}
                placeholder={t('域名、IP 或 gitea.example.com')}
                value={draft}
                disabled={busy || !granted}
                onChange={(event) => onDraftChange(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') add()
                }}
              />
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={busy || !draft.trim() || !granted}
                onClick={add}
              >
                {t('添加域名')}
              </Button>
            </div>
          </section>
        )}
      </div>
    </Modal>
  )
}

/** Commands are resolved from PATH by the host, so each one is requested by
 * name and flagged when it is missing on this machine. */
export function CommandDialog({
  plugin,
  busy,
  draft,
  onDraftChange,
  onToggle,
  onAdd,
  onClose,
}: {
  plugin: PluginInfo
  busy: boolean
  draft: string
  onDraftChange: (value: string) => void
  onToggle: (command: string, granted: boolean) => void
  onAdd: (command: string) => void
  onClose: () => void
}) {
  const granted = plugin.grantedPermissions.includes('commands')
  const commands = pluginCommands(plugin)
  const add = () => {
    const command = draft.trim()
    if (command) onAdd(command)
  }
  return (
    <Modal open onClose={onClose} title={t('外部命令')} className="plugin-domain-modal">
      <div className="plugin-domain-page">
        <p className="plugin-domain-hint">
          {t('外部命令由宿主从 PATH 解析执行，可能访问授权目录以外的资源。')}
        </p>
        <section className="plugin-permission-section plugin-domain-section">
          <div className="plugin-domain-list">
            {commands.map((command) => {
              const allowed = (plugin.allowedCommands || []).includes(command)
              const requested = (plugin.requestedCommands || []).includes(command)
              const missing = (plugin.missingCommands || []).includes(command)
              return (
                <div
                  className={`plugin-domain-row ${missing ? 'plugin-command-row-missing' : ''}`}
                  key={command}
                >
                  <span>
                    <code>{command}</code>
                    {requested && <small>{t('插件申请')}</small>}
                    {!allowed && <small className="muted">{t('未授权')}</small>}
                    {missing && <small className="plugin-command-missing">{t('本地未找到')}</small>}
                  </span>
                  <button
                    type="button"
                    role="switch"
                    aria-checked={allowed}
                    aria-label={allowed ? t('移除命令 {0}', command) : t('授权命令 {0}', command)}
                    title={allowed ? t('移除命令') : t('授权命令')}
                    className={`plugin-switch ${allowed ? 'on' : ''}`}
                    disabled={busy || !granted}
                    onClick={() => onToggle(command, !allowed)}
                  >
                    <span />
                  </button>
                </div>
              )
            })}
            {!commands.length && <p className="sidebar-hint">{t('尚未允许任何命令。')}</p>}
          </div>
          <div className="plugin-domain-add">
            <input
              aria-label={t('添加允许的命令')}
              placeholder={t('输入可执行文件名称')}
              value={draft}
              disabled={busy || !granted}
              onChange={(event) => onDraftChange(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') add()
              }}
            />
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={busy || !draft.trim() || !granted}
              onClick={add}
            >
              {t('添加命令')}
            </Button>
          </div>
        </section>
      </div>
    </Modal>
  )
}

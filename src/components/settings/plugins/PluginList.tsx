import { t } from '@/i18n'
import { Download, Puzzle, ScrollText, Settings2, ShieldCheck, Trash2 } from 'lucide-react'
import type { PluginInfo } from '@/lib/plugins'
import { Button } from '@/components/ui/button'
import { pluginIcon } from '../PluginInstaller'

/**
 * Installed plugins, one row per entry: identity, consent state, per-plugin
 * actions and the activation switch. All runtime state and mutations stay in
 * the page; this section only renders and forwards the intent.
 */
export function PluginList({
  plugins,
  busy,
  onOpenSettings,
  onOpenDetails,
  onOpenLogs,
  onRemove,
  onToggleActivation,
  onBrowseStore,
}: {
  plugins: PluginInfo[]
  busy: boolean
  onOpenSettings: (plugin: PluginInfo) => void
  onOpenDetails: (plugin: PluginInfo) => void
  onOpenLogs: (plugin: PluginInfo) => void
  onRemove: (plugin: PluginInfo) => void
  onToggleActivation: (plugin: PluginInfo, enabled: boolean) => void
  onBrowseStore: () => void
}) {
  if (!plugins.length) return <PluginEmptyState onBrowseStore={onBrowseStore} />
  return (
    <div className="plugin-list">
      {plugins.map((plugin) => {
        const enabled = plugin.enabled && plugin.approved
        const hasSettings = (plugin.extensions?.settings || []).length > 0
        return (
          <article className="plugin-row" key={plugin.id}>
            <div className={`plugin-row-icon ${enabled ? 'enabled' : ''}`}>
              {pluginIcon(plugin.icon)}
            </div>
            <div className="plugin-row-main">
              <div className="plugin-row-title">
                <strong>{plugin.name}</strong>
                <span className="plugin-pill">v{plugin.version}</span>
                <span className={`plugin-state ${enabled ? 'active' : ''}`}>
                  <span />
                  {enabled ? t('运行中') : t('未激活')}
                </span>
                {!plugin.backendAvailable && (
                  <span className="plugin-pill warn">{t('当前平台无后端')}</span>
                )}
              </div>
              <p>{plugin.description || t('第三方插件')}</p>
            </div>
            <div className="plugin-row-actions">
              <div className="plugin-row-buttons">
                {hasSettings && (
                  <Button
                    type="button"
                    size="icon-sm"
                    variant="ghost"
                    aria-label={t('设置 {0}', plugin.name)}
                    title={t('插件设置')}
                    onClick={() => onOpenSettings(plugin)}
                  >
                    <Settings2 size={15} />
                  </Button>
                )}
                <Button
                  type="button"
                  size="icon-sm"
                  variant="ghost"
                  aria-label={t('查看 {0} 的日志', plugin.name)}
                  title={t('插件日志')}
                  onClick={() => onOpenLogs(plugin)}
                >
                  <ScrollText size={14} />
                </Button>
                <Button
                  type="button"
                  size="icon-sm"
                  variant="ghost"
                  aria-label={t('插件详情')}
                  title={t('插件详情')}
                  onClick={() => onOpenDetails(plugin)}
                >
                  <ShieldCheck size={14} />
                </Button>
                <Button
                  type="button"
                  size="icon-sm"
                  variant="ghost"
                  className="plugin-row-remove"
                  aria-label={t('卸载 {0}', plugin.name)}
                  title={t('卸载插件')}
                  disabled={busy}
                  onClick={() => onRemove(plugin)}
                >
                  <Trash2 size={14} />
                </Button>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={enabled}
                aria-label={`${enabled ? t('停用') : t('激活')} ${plugin.name}`}
                className={`plugin-switch ${enabled ? 'on' : ''}`}
                disabled={busy || !plugin.backendAvailable}
                onClick={() => void onToggleActivation(plugin, !enabled)}
              >
                <span />
              </button>
            </div>
          </article>
        )
      })}
    </div>
  )
}

/** Nothing installed yet: a short pitch plus the store entry point. */
function PluginEmptyState({ onBrowseStore }: { onBrowseStore: () => void }) {
  return (
    <div className="plugin-empty">
      <div className="plugin-empty-icon">
        <Puzzle size={26} />
      </div>
      <strong>{t('用插件扩展 Fiex')}</strong>
      <p>{t('按需添加连接、文件工具和工作流，所有插件都需要你的授权。')}</p>
      <Button type="button" onClick={onBrowseStore}>
        <Download size={15} />
        {t('浏览官方插件')}
      </Button>
    </div>
  )
}

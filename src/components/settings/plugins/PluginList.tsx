import { Download, Puzzle, Settings2, ShieldCheck, Trash2 } from 'lucide-react'
import { PLUGIN_CATEGORIES, pluginCategories, type PluginInfo } from '@/lib/plugins'
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
  onRemove,
  onToggleActivation,
  onBrowseStore,
}: {
  plugins: PluginInfo[]
  busy: boolean
  onOpenSettings: (plugin: PluginInfo) => void
  onOpenDetails: (plugin: PluginInfo) => void
  onRemove: (plugin: PluginInfo) => void
  onToggleActivation: (plugin: PluginInfo, enabled: boolean) => void
  onBrowseStore: () => void
}) {
  if (!plugins.length) return <PluginEmptyState onBrowseStore={onBrowseStore} />
  return (
    <div className="plugin-list">
      {plugins.map((plugin) => {
        const declared = plugin.declaredPermissions || []
        const enabled = plugin.enabled && plugin.approved
        const hasSettings = (plugin.extensions?.settings || []).length > 0
        const categories = pluginCategories(plugin.extensions, plugin.requestedCommands)
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
                  {enabled ? '运行中' : '未激活'}
                </span>
                {!plugin.backendAvailable && (
                  <span className="plugin-pill warn">当前平台无后端</span>
                )}
              </div>
              <p>{plugin.description || '第三方插件'}</p>
              <div className="plugin-row-meta">
                {categories.slice(0, 3).map((category) => (
                  <span className="plugin-category-pill" key={category}>
                    {PLUGIN_CATEGORIES.find((entry) => entry.id === category)?.label || category}
                  </span>
                ))}
                <span>{plugin.author?.name || '未知开发者'}</span>
                <span aria-hidden>·</span>
                <span>{declared.length ? `${declared.length} 项权限` : '无需宿主权限'}</span>
                {plugin.extensions?.connections ? (
                  <>
                    <span aria-hidden>·</span>
                    <span>连接提供者</span>
                  </>
                ) : null}
              </div>
            </div>
            <div className="plugin-row-actions">
              <div className="plugin-row-buttons">
                {hasSettings && (
                  <Button
                    type="button"
                    size="icon-sm"
                    variant="ghost"
                    aria-label={`设置 ${plugin.name}`}
                    title="插件设置"
                    onClick={() => onOpenSettings(plugin)}
                  >
                    <Settings2 size={15} />
                  </Button>
                )}
                <Button
                  type="button"
                  size="icon-sm"
                  variant="ghost"
                  aria-label="插件详情"
                  title="插件详情"
                  onClick={() => onOpenDetails(plugin)}
                >
                  <ShieldCheck size={14} />
                </Button>
                <Button
                  type="button"
                  size="icon-sm"
                  variant="ghost"
                  className="plugin-row-remove"
                  aria-label={`卸载 ${plugin.name}`}
                  title="卸载插件"
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
                aria-label={`${enabled ? '停用' : '激活'} ${plugin.name}`}
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
export function PluginEmptyState({ onBrowseStore }: { onBrowseStore: () => void }) {
  return (
    <div className="plugin-empty">
      <div className="plugin-empty-icon">
        <Puzzle size={26} />
      </div>
      <strong>用插件扩展 Fiex</strong>
      <p>按需添加连接、文件工具和工作流，所有插件都需要你的授权。</p>
      <Button type="button" onClick={onBrowseStore}>
        <Download size={15} />
        浏览官方插件
      </Button>
    </div>
  )
}

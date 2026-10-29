import { t } from '@/i18n'
import { Modal } from '@/components/ui/Modal'
import type { PluginInfo } from '@/lib/plugins'
import { PluginLogsSection } from './PluginLogsSection'

/** Per-plugin runtime log. The recent errors used to live at the bottom of the
 * permissions detail; they now have their own entry point on every row. */
export function PluginLogsSheet({
  plugin,
  onClose,
}: {
  plugin: PluginInfo | null
  onClose: () => void
}) {
  return (
    <Modal
      open={Boolean(plugin)}
      onClose={onClose}
      title={plugin ? t('{0} · 日志', plugin.name) : t('插件日志')}
      className="plugin-permissions-modal plugin-logs-modal"
    >
      {plugin && (
        <div className="plugin-permissions-page plugin-logs-page">
          <PluginLogsSection pluginId={plugin.id} />
        </div>
      )}
    </Modal>
  )
}

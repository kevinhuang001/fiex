import { t } from '@/i18n'
import { Modal } from '@/components/ui/Modal'
import { PluginLogViewer } from './PluginLogViewer'

export function CoreLogsSheet({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('Core 运行日志')}
      className="plugin-permissions-modal plugin-logs-modal"
    >
      {open && (
        <div className="plugin-permissions-page plugin-logs-page">
          <PluginLogViewer pluginId="core" />
        </div>
      )}
    </Modal>
  )
}

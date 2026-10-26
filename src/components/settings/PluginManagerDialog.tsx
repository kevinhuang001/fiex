import { t } from '@/i18n'
import { Modal } from '@/components/ui/Modal'
import { PluginSettings } from './PluginSettings'

/**
 * Plugin management is a first-class toolbar destination, not a section of
 * the general settings dialog. The page owns its store, permission and
 * activation dialogs, so this shell only provides the large scroll surface.
 */
export function PluginManagerDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Modal open={open} onClose={onClose} title={t('插件')} className="plugin-manager-modal">
      <div className="plugin-manager-page">
        <PluginSettings />
      </div>
    </Modal>
  )
}

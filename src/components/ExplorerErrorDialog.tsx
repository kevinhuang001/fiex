import { t } from '@/i18n'
import { AlertCircle } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'

/** The single desktop error channel renders as a modal, never as a banner.
 * Inline field validation inside a form stays local to that form. */
export function ExplorerErrorDialog({
  error,
  onClose,
  diagnosticId,
}: {
  error: string
  onClose: () => void
  onError?: (error: string) => void
  diagnosticId?: string
}) {
  return (
    <Modal open={Boolean(error)} onClose={onClose} title={t('错误')}>
      <div className="error-dialog" role="alert">
        <AlertCircle size={18} />
        <p>{error}</p>
      </div>
      <div className="dialog-actions">
        {diagnosticId && (
          <Button
            variant="secondary"
            onClick={() => void navigator.clipboard.writeText(diagnosticId).catch(() => {})}
          >
            {diagnosticId}
          </Button>
        )}
        <Button variant="ghost" onClick={onClose}>
          {t('确定')}
        </Button>
      </div>
    </Modal>
  )
}

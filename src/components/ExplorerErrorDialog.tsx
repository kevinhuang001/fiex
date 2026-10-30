import { t } from '@/i18n'
import { AlertCircle } from 'lucide-react'
import { call } from '@/lib/api'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'

/** The single desktop error channel renders as a modal, never as a banner.
 * Inline field validation inside a form stays local to that form. */
export function ExplorerErrorDialog({
  error,
  onClose,
  onError,
}: {
  error: string
  onClose: () => void
  onError: (error: string) => void
}) {
  const elevated = navigator.userAgent.includes('Windows') && /管理员/.test(error)
  return (
    <Modal open={Boolean(error)} onClose={onClose} title={t('错误')}>
      <div className="error-dialog" role="alert">
        <AlertCircle size={18} />
        <p>{error}</p>
      </div>
      <div className="dialog-actions">
        {elevated && (
          <Button
            variant="secondary"
            onClick={() => {
              void call('relaunch_elevated').catch((cause) => onError(String(cause)))
            }}
          >
            {t('以管理员身份重启')}
          </Button>
        )}
        <Button variant="ghost" onClick={onClose}>
          {t('确定')}
        </Button>
      </div>
    </Modal>
  )
}

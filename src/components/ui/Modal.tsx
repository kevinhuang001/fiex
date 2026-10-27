import { t } from '@/i18n'
import * as Dialog from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import type { CSSProperties, ReactNode } from 'react'
import { overlayRoot } from '@/lib/portal'
export function Modal({
  open,
  onClose,
  title,
  children,
  wide = false,
  className,
  contentStyle,
  bare = false,
  actions,
}: {
  open: boolean
  onClose: () => void
  title: string
  children: ReactNode
  wide?: boolean
  className?: string
  contentStyle?: CSSProperties
  bare?: boolean
  actions?: ReactNode
}) {
  return (
    <Dialog.Root open={open} onOpenChange={(v) => !v && onClose()}>
      <Dialog.Portal container={overlayRoot()}>
        <Dialog.Overlay className="modal-overlay" />
        <Dialog.Content
          className={`modal ${wide ? 'modal-wide' : ''} ${className || ''}`}
          style={contentStyle}
          onOpenAutoFocus={(e) => {
            if (wide || bare) e.preventDefault()
          }}
          aria-describedby={undefined}
        >
          {bare ? (
            <>
              <Dialog.Title className="visually-hidden">{title}</Dialog.Title>
              <Dialog.Close className="icon-button modal-close" aria-label={t('关闭')}>
                <X size={16} />
              </Dialog.Close>
            </>
          ) : (
            <header>
              <Dialog.Title>{title}</Dialog.Title>
              {actions && <div className="modal-actions">{actions}</div>}
              <Dialog.Close className="icon-button" aria-label={t('关闭')}>
                <X size={17} />
              </Dialog.Close>
            </header>
          )}
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

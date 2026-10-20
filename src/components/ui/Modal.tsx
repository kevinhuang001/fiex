import * as Dialog from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import type { ReactNode } from 'react'
export function Modal({
  open,
  onClose,
  title,
  description,
  children,
  wide = false,
  className,
  bare = false,
}: {
  open: boolean
  onClose: () => void
  title: string
  description?: string
  children: ReactNode
  wide?: boolean
  className?: string
  bare?: boolean
}) {
  return (
    <Dialog.Root open={open} onOpenChange={(v) => !v && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="modal-overlay" />
        <Dialog.Content
          className={`modal ${wide ? 'modal-wide' : ''} ${className || ''}`}
          onOpenAutoFocus={(e) => {
            if (wide || bare) e.preventDefault()
          }}
          aria-describedby={description ? 'modal-description' : undefined}
        >
          {bare ? (
            <>
              <Dialog.Title className="visually-hidden">{title}</Dialog.Title>
              <Dialog.Close className="icon-button modal-close" aria-label="关闭">
                <X size={16} />
              </Dialog.Close>
            </>
          ) : (
            <header>
              <Dialog.Title>{title}</Dialog.Title>
              <Dialog.Close className="icon-button" aria-label="关闭">
                <X size={17} />
              </Dialog.Close>
            </header>
          )}
          {description && (
            <Dialog.Description id="modal-description">{description}</Dialog.Description>
          )}
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

import { t } from '@/i18n'
import type { RefObject } from 'react'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import type { Entry } from '@/lib/types'

export type PickerDialogState =
  | { kind: 'folder'; value: string }
  | { kind: 'rename'; entry: Entry; value: string }
  | { kind: 'trash'; entry: Entry }

export function PickerDialogs({
  dialog,
  replace,
  saveName,
  busy,
  inputRef,
  actions,
}: {
  dialog: PickerDialogState | null
  replace: boolean
  saveName: string
  busy: boolean
  inputRef: RefObject<HTMLInputElement | null>
  actions: {
    closeDialog(): void
    setDialogValue(value: string): void
    createFolder(name: string): void
    rename(entry: Entry, name: string): void
    trash(entry: Entry): void
    confirmReplace(): void
    cancelReplace(): void
  }
}) {
  const editable = dialog && dialog.kind !== 'trash' ? dialog : null
  return (
    <>
      <Modal
        open={Boolean(editable)}
        onClose={actions.closeDialog}
        title={dialog?.kind === 'rename' ? t('重命名') : t('新建文件夹')}
        className="file-picker-confirm"
      >
        {editable && (
          <>
            <label className="file-picker-dialog-field">
              <span>{dialog?.kind === 'rename' ? t('新名称') : t('文件夹名称')}</span>
              <input
                ref={inputRef}
                value={editable.value}
                onChange={(event) => actions.setDialogValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key !== 'Enter') return
                  event.preventDefault()
                  if (editable.kind === 'rename') actions.rename(editable.entry, editable.value)
                  else actions.createFolder(editable.value)
                }}
              />
            </label>
            <footer className="file-picker-confirm-actions">
              <Button variant="outline" onClick={actions.closeDialog}>
                {t('取消')}
              </Button>
              <Button
                disabled={busy || !editable.value.trim()}
                onClick={() => {
                  if (editable.kind === 'rename') actions.rename(editable.entry, editable.value)
                  else actions.createFolder(editable.value)
                }}
              >
                {editable.kind === 'rename' ? t('重命名') : t('新建')}
              </Button>
            </footer>
          </>
        )}
      </Modal>
      <Modal
        open={dialog?.kind === 'trash'}
        onClose={actions.closeDialog}
        title={t('移到废纸篓？')}
        className="file-picker-confirm"
      >
        {dialog?.kind === 'trash' && (
          <>
            <p className="file-picker-confirm-text">
              {t('将“{0}”移到废纸篓，可以再恢复。', dialog.entry.name)}
            </p>
            <footer className="file-picker-confirm-actions">
              <Button variant="outline" onClick={actions.closeDialog}>
                {t('取消')}
              </Button>
              <Button onClick={() => actions.trash(dialog.entry)}>{t('移到废纸篓')}</Button>
            </footer>
          </>
        )}
      </Modal>
      <Modal
        open={replace}
        onClose={actions.cancelReplace}
        title={t('替换文件？')}
        className="file-picker-confirm"
      >
        <p className="file-picker-confirm-text">
          {t('“{0}”已存在。替换会覆盖原文件，此操作不可撤销。', saveName.trim())}
        </p>
        <footer className="file-picker-confirm-actions">
          <Button variant="outline" onClick={actions.cancelReplace}>
            {t('取消')}
          </Button>
          <Button onClick={actions.confirmReplace}>{t('替换')}</Button>
        </footer>
      </Modal>
    </>
  )
}

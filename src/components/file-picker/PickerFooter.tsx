import { t } from '@/i18n'
import type { RefObject } from 'react'
import { Button } from '@/components/ui/button'
import type { FilePickerMode } from '@/lib/filePicker'
import type { Entry } from '@/lib/types'

export function PickerFooter({
  mode,
  saveName,
  onSaveName,
  nameInput,
  selectedEntries,
  canConfirm,
  onCancel,
  onConfirm,
}: {
  mode: FilePickerMode
  saveName: string
  onSaveName(value: string): void
  nameInput: RefObject<HTMLInputElement | null>
  selectedEntries: Entry[]
  canConfirm: boolean
  onCancel(): void
  onConfirm(): void
}) {
  return (
    <footer className="file-picker-footer">
      {mode === 'saveFile' ? (
        <label className="file-picker-name">
          <span>{t('存储为')}</span>
          <input
            ref={nameInput}
            value={saveName}
            onChange={(event) => onSaveName(event.target.value)}
            placeholder={t('文件名')}
            aria-label={t('文件名')}
          />
        </label>
      ) : (
        <span className="file-picker-selection">
          {selectedEntries.length
            ? selectedEntries.map((entry) => entry.name).join('、')
            : t('未选择')}
        </span>
      )}
      <div className="file-picker-actions">
        <Button variant="outline" onClick={onCancel}>
          {t('取消')}
        </Button>
        <Button disabled={!canConfirm} onClick={onConfirm}>
          {mode === 'saveFile' ? t('保存') : t('打开')}
        </Button>
      </div>
    </footer>
  )
}

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
          <span>存储为</span>
          <input
            ref={nameInput}
            value={saveName}
            onChange={(event) => onSaveName(event.target.value)}
            placeholder="文件名"
            aria-label="文件名"
          />
        </label>
      ) : (
        <span className="file-picker-selection">
          {selectedEntries.length
            ? selectedEntries.map((entry) => entry.name).join('、')
            : '未选择'}
        </span>
      )}
      <div className="file-picker-actions">
        <Button variant="outline" onClick={onCancel}>
          取消
        </Button>
        <Button disabled={!canConfirm} onClick={onConfirm}>
          {mode === 'saveFile' ? '保存' : '打开'}
        </Button>
      </div>
    </footer>
  )
}

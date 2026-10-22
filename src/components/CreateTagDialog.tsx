import { useState } from 'react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { ColorPicker } from './ui/ColorPicker'
import { TAG_PALETTE, nearestFinderColor, tagHex } from '@/lib/tags'
import type { Tag } from '@/lib/types'
export function CreateTagDialog({
  onClose,
  onCreate,
  existing,
  editing,
}: {
  onClose: () => void
  onCreate: (tag: Tag) => void
  existing: Tag[]
  editing?: Tag
}) {
  const [name, setName] = useState(editing?.name ?? '')
  const [hex, setHex] = useState(editing ? tagHex(editing) : '#007aff')
  const duplicate = existing.some((tag) => tag.name === name.trim())
  const choose = (value: string) => setHex(value.toLowerCase())
  return (
    <Modal open onClose={onClose} title={editing ? '编辑标签' : '添加标签'}>
      <form
        className="create-tag-form"
        onSubmit={(e) => {
          e.preventDefault()
          if (name.trim() && !duplicate)
            onCreate({ name: name.trim(), color: nearestFinderColor(hex), hex })
        }}
      >
        <input
          autoFocus
          className="text-field"
          aria-label="标签名称"
          placeholder="标签名称"
          maxLength={40}
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <span className="create-tag-color-label">颜色</span>
        <ColorPicker value={hex} onChange={choose} />
        <div className="tag-colors" aria-label="常用颜色">
          {TAG_PALETTE.map((swatch) => (
            <button
              type="button"
              key={swatch}
              aria-label={`选择颜色 ${swatch}`}
              aria-pressed={hex === swatch}
              className={hex === swatch ? 'active' : ''}
              style={{ background: swatch }}
              onClick={() => choose(swatch)}
            />
          ))}
        </div>
        {duplicate && <p role="alert">已有同名标签</p>}
        <div className="dialog-actions">
          <Button type="button" variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button type="submit" disabled={!name.trim() || duplicate}>
            {editing ? '保存' : '添加'}
          </Button>
        </div>
      </form>
    </Modal>
  )
}

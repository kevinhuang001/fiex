import { useState } from 'react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { tagColor } from '@/lib/tags'
import type { Tag } from '@/lib/types'
export function CreateTagDialog({
  onClose,
  onCreate,
  existing,
}: {
  onClose: () => void
  onCreate: (tag: Tag) => void
  existing: Tag[]
}) {
  const [name, setName] = useState('')
  const [color, setColor] = useState(4)
  const duplicate = existing.some((tag) => tag.name === name.trim())
  return (
    <Modal open onClose={onClose} title="添加标签">
      <form
        className="create-tag-form"
        onSubmit={(e) => {
          e.preventDefault()
          if (name.trim() && !duplicate) onCreate({ name: name.trim(), color })
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
        <div className="tag-colors" aria-label="标签颜色">
          {[1, 2, 3, 4, 5, 6, 7].map((index) => (
            <button
              type="button"
              key={index}
              aria-label={`选择颜色 ${index}`}
              aria-pressed={color === index}
              className={color === index ? 'active' : ''}
              style={{ background: tagColor(index) }}
              onClick={() => setColor(index)}
            />
          ))}
        </div>
        {duplicate && <p role="alert">已有同名标签</p>}
        <div className="dialog-actions">
          <Button type="button" variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button type="submit" disabled={!name.trim() || duplicate}>
            添加
          </Button>
        </div>
      </form>
    </Modal>
  )
}

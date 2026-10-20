import { useEffect, useState } from 'react'
import { Plus, X } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { TAG_PRESETS, tagColor } from '@/lib/tags'
import type { Tag } from '@/lib/types'

interface Props {
  open: boolean
  initial: Tag[]
  onClose: () => void
  onApply: (tags: Tag[]) => void
}

export function TagDialog({ open, initial, onClose, onApply }: Props) {
  const [tags, setTags] = useState<Tag[]>(initial)
  const [name, setName] = useState('')
  const [color, setColor] = useState(4)

  useEffect(() => {
    if (!open) return
    setTags(initial)
    setName('')
    setColor(4)
  }, [open])

  const toggle = (tag: Tag) => {
    setTags((current) =>
      current.some((item) => item.name === tag.name && item.color === tag.color)
        ? current.filter((item) => !(item.name === tag.name && item.color === tag.color))
        : [...current, tag],
    )
  }

  const add = () => {
    const value = name.trim()
    if (!value) return
    if (!tags.some((tag) => tag.name === value)) setTags([...tags, { name: value, color }])
    setName('')
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="标签"
      description="为所选项目设置 Finder 标签；同一标签会显示在列表和图标视图中。"
    >
      <div className="tag-dialog">
        <div className="tag-presets">
          {TAG_PRESETS.map((preset) => {
            const active = tags.some(
              (tag) => tag.name === preset.name && tag.color === preset.color,
            )
            return (
              <button
                key={`${preset.name}:${preset.color}`}
                className={`tag-preset ${active ? 'active' : ''}`}
                aria-pressed={active}
                onClick={() => toggle(preset)}
              >
                <i style={{ background: preset.hex }} />
                {preset.name}
              </button>
            )
          })}
        </div>

        {tags.length > 0 && (
          <div className="tag-chips">
            {tags.map((tag) => (
              <span className="tag-chip" key={`${tag.name}:${tag.color}`}>
                <i style={{ background: tagColor(tag.color) }} />
                {tag.name}
                <button aria-label={`移除标签 ${tag.name}`} onClick={() => toggle(tag)}>
                  <X size={10} />
                </button>
              </span>
            ))}
          </div>
        )}

        <div className="tag-custom">
          <input
            className="text-field"
            aria-label="自定义标签名称"
            placeholder="自定义标签"
            value={name}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault()
                add()
              }
            }}
          />
          <div className="tag-colors" aria-label="标签颜色">
            {[1, 2, 3, 4, 5, 6, 7].map((index) => (
              <button
                key={index}
                aria-label={`选择颜色 ${index}`}
                aria-pressed={color === index}
                className={color === index ? 'active' : ''}
                style={{ background: tagColor(index) }}
                onClick={() => setColor(index)}
              />
            ))}
          </div>
          <Button variant="secondary" onClick={add} disabled={!name.trim()}>
            <Plus />
            添加
          </Button>
        </div>

        <div className="dialog-actions">
          <Button variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button onClick={() => onApply(tags)}>应用标签</Button>
        </div>
      </div>
    </Modal>
  )
}

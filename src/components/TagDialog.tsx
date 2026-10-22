import { useEffect, useState } from 'react'
import { X } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { tagHex } from '@/lib/tags'
import type { Tag } from '@/lib/types'

interface Props {
  open: boolean
  initial: Tag[]
  available?: Tag[]
  onClose: () => void
  onApply: (tags: Tag[]) => void
}

export function TagDialog({ open, initial, available = [], onClose, onApply }: Props) {
  const [tags, setTags] = useState<Tag[]>(initial)

  useEffect(() => {
    if (!open) return
    setTags(initial)
  }, [open])

  const toggle = (tag: Tag) => {
    setTags((current) =>
      current.some((item) => item.name === tag.name && item.color === tag.color)
        ? current.filter((item) => !(item.name === tag.name && item.color === tag.color))
        : [...current, tag],
    )
  }

  return (
    <Modal open={open} onClose={onClose} title="标签">
      <div className="tag-dialog">
        <div className="tag-presets">
          {Array.from(
            new Map(
              [...available, ...initial].map((tag) => [`${tag.name}:${tag.color}`, tag]),
            ).values(),
          ).map((preset) => {
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
                <i style={{ background: tagHex(preset) }} />
                {preset.name}
              </button>
            )
          })}
        </div>

        {tags.length > 0 && (
          <div className="tag-chips">
            {tags.map((tag) => (
              <span className="tag-chip" key={`${tag.name}:${tag.color}`}>
                <i style={{ background: tagHex(tag) }} />
                {tag.name}
                <button aria-label={`移除标签 ${tag.name}`} onClick={() => toggle(tag)}>
                  <X size={10} />
                </button>
              </span>
            ))}
          </div>
        )}

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

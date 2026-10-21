import { tagHex } from '@/lib/tags'
import type { Tag } from '@/lib/types'

export function TagDots({ tags, size = 7 }: { tags: Tag[]; size?: number }) {
  if (!tags.length) return null
  return (
    <span className="tag-dots" title={tags.map((tag) => tag.name).join('、')}>
      {tags.slice(0, 5).map((tag, index) => (
        <i
          key={`${tag.name}:${tag.color}:${index}`}
          style={{ width: size, height: size, background: tagHex(tag) }}
        />
      ))}
    </span>
  )
}

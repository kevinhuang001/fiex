import { CloudCheck, CloudDownload, CloudUpload, Pin } from 'lucide-react'
import type { DecorationBadge } from '@/lib/types'

const icons = {
  'cloud-check': CloudCheck,
  'cloud-download': CloudDownload,
  'cloud-upload': CloudUpload,
  pin: Pin,
}
const presentation = (badge: DecorationBadge) =>
  badge.dot ? 'dots' : badge.status ? 'status' : 'plain'

/** Presentation only: providers decide which badges to emit. Group consecutive
 * dots/status pills so view layout does not change their internal spacing. */
export function DecorationBadges({ badges = [] }: { badges?: DecorationBadge[] }) {
  const groups: DecorationBadge[][] = []
  for (const badge of badges) {
    const previous = groups.at(-1)
    if (previous && presentation(previous[0]) === presentation(badge)) previous.push(badge)
    else groups.push([badge])
  }
  return (
    <>
      {groups.map((group, index) => {
        if (group[0].dot)
          return (
            <span
              key={index}
              className="tag-dots"
              title={group.map((badge) => badge.tooltip || badge.text).join('、')}
            >
              {group.map((badge, index) => (
                <i key={index} style={{ width: 7, height: 7, background: badge.color }} />
              ))}
            </span>
          )
        const rendered = group.map((badge, index) => {
          const Icon = badge.icon && icons[badge.icon]
          return (
            <span
              key={index}
              className={
                badge.status
                  ? `sync-badge ${badge.status}`
                  : `decoration-badge ${badge.color.startsWith('#') ? 'custom' : badge.color}`
              }
              style={
                badge.color.startsWith('#')
                  ? { color: badge.color, borderColor: badge.color }
                  : undefined
              }
              title={badge.tooltip}
            >
              {Icon && <Icon size={11} fill={badge.icon === 'pin' ? 'currentColor' : 'none'} />}
              <span>{badge.text}</span>
            </span>
          )
        })
        return (
          <span key={index} className={group[0].status ? 'sync-badges' : 'decoration-badges'}>
            {rendered}
          </span>
        )
      })}
    </>
  )
}

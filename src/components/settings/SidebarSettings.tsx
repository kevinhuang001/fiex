import { commonPlaces as getCommonPlaces } from '@/lib/places'
import type { Location, Preferences, SidebarSection } from '@/lib/types'
import {
  AppWindow,
  Clock,
  Trash2,
  Download,
  FileText,
  Film,
  Folder,
  GripVertical,
  Home,
  Image,
  Monitor,
  Music,
  RotateCcw,
} from 'lucide-react'
import { useState } from 'react'
function placeIcon(name: string) {
  if (name === '最近使用') return <Clock size={14} />
  if (name === '废纸篓') return <Trash2 size={14} />
  if (name === '个人文件夹') return <Home size={14} />
  if (name === '桌面') return <Monitor size={14} />
  if (name === '下载') return <Download size={14} />
  if (name === '图片') return <Image size={14} />
  if (name === '音乐') return <Music size={14} />
  if (name === '视频') return <Film size={14} />
  if (name === '文稿') return <FileText size={14} />
  if (name === '应用程序') return <AppWindow size={14} />
  return <Folder size={14} />
}

const SIDEBAR_LABELS: Record<SidebarSection, string> = {
  places: '常用位置',
  favorites: '收藏',
  tags: '标签',
  devices: '设备',
}

export function SidebarSettings({
  prefs,
  update,
  locations,
}: {
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
  locations: Location[]
}) {
  const [draggingRow, setDraggingRow] = useState<string | null>(null)
  const [dropRow, setDropRow] = useState<string | null>(null)
  const reorder = <T,>(items: T[], from: number, to: number) => {
    const next = [...items]
    const [item] = next.splice(from, 1)
    next.splice(to, 0, item)
    return next
  }
  const beginReorder = (event: React.PointerEvent, kind: 'section' | 'place', index: number) => {
    if ((event.target as Element).closest('input,button')) return
    const startX = event.clientX
    const startY = event.clientY
    let active = false
    let target = index
    const move = (moveEvent: PointerEvent) => {
      if (!active) {
        if (Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY) < 5) return
        active = true
        setDraggingRow(`${kind}:${index}`)
      }
      moveEvent.preventDefault()
      const row = document
        .elementFromPoint(moveEvent.clientX, moveEvent.clientY)
        ?.closest<HTMLElement>(`[data-reorder="${kind}"]`)
      if (row) {
        const next = Number(row.dataset.reorderIndex)
        if (Number.isFinite(next)) {
          target = next
          setDropRow(`${kind}:${next}`)
        }
      }
    }
    const finish = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', finish)
      window.removeEventListener('pointercancel', finish)
      setDraggingRow(null)
      setDropRow(null)
      if (!active || target === index) return
      if (kind === 'section') {
        update({ sidebarOrder: reorder(prefs.sidebarOrder, index, target) })
      } else {
        const keys = orderedPlaces.map((location) => location.path || location.name)
        update({ placeOrder: reorder(keys, index, target) })
      }
    }
    window.addEventListener('pointermove', move, { passive: false })
    window.addEventListener('pointerup', finish)
    window.addEventListener('pointercancel', finish)
  }

  const commonPlaces = getCommonPlaces(locations)
  const placeKeys = commonPlaces.map((location) => location.path || location.name)
  const orderedKeys = [
    ...prefs.placeOrder.filter((key) => placeKeys.includes(key)),
    ...placeKeys.filter((key) => !prefs.placeOrder.includes(key)),
  ]
  const orderedPlaces = orderedKeys
    .map((key) => commonPlaces.find((location) => (location.path || location.name) === key))
    .filter((location): location is Location => Boolean(location))

  return (
    <>
      <div className="settings-card">
        <div className="settings-card-header">
          <span>区域顺序</span>
          <button
            className="settings-reset"
            onClick={() =>
              update({
                sidebarOrder: ['places', 'favorites', 'tags', 'devices'],
                placeOrder: [],
                hiddenPlaces: [],
              })
            }
          >
            <RotateCcw size={11} />
            恢复预设
          </button>
        </div>
        <div className="sidebar-order">
          {prefs.sidebarOrder.map((id, index) => (
            <div
              className={`sidebar-order-row ${draggingRow === `section:${index}` ? 'dragging' : ''} ${dropRow === `section:${index}` ? 'reorder-target' : ''}`}
              key={id}
              data-reorder="section"
              data-reorder-index={index}
              onPointerDown={(event) => beginReorder(event, 'section', index)}
            >
              <GripVertical size={13} className="drag-handle" />
              <span>{SIDEBAR_LABELS[id]}</span>
            </div>
          ))}
        </div>
      </div>
      <div className="settings-card">
        <div className="settings-card-header">
          <span>常用位置</span>
        </div>
        <div className="place-order">
          {orderedPlaces.map((location, index) => {
            const key = location.path || location.name
            const hidden = prefs.hiddenPlaces.includes(key)
            return (
              <div
                className={`place-row ${draggingRow === `place:${index}` ? 'dragging' : ''} ${dropRow === `place:${index}` ? 'reorder-target' : ''}`}
                key={key}
                data-reorder="place"
                data-reorder-index={index}
                onPointerDown={(event) => beginReorder(event, 'place', index)}
              >
                <GripVertical size={13} className="drag-handle" />
                <span className="settings-choice-icon place-icon">{placeIcon(location.name)}</span>
                <span className="place-name">{location.name}</span>
                <input
                  type="checkbox"
                  checked={!hidden}
                  aria-label={`显示${location.name}`}
                  onChange={() =>
                    update({
                      hiddenPlaces: hidden
                        ? prefs.hiddenPlaces.filter((item) => item !== key)
                        : [...prefs.hiddenPlaces, key],
                    })
                  }
                />
              </div>
            )
          })}
        </div>
      </div>
    </>
  )
}

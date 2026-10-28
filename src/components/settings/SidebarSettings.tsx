import { t } from '@/i18n'
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
import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
function placeIcon(name: string) {
  if (name === t('最近使用')) return <Clock size={14} />
  if (name === t('废纸篓') || name === t('回收站')) return <Trash2 size={14} />
  if (name === t('个人文件夹')) return <Home size={14} />
  if (name === t('桌面')) return <Monitor size={14} />
  if (name === t('下载')) return <Download size={14} />
  if (name === t('图片')) return <Image size={14} />
  if (name === t('音乐')) return <Music size={14} />
  if (name === t('视频')) return <Film size={14} />
  if (name === t('文稿')) return <FileText size={14} />
  if (name === t('应用程序')) return <AppWindow size={14} />
  return <Folder size={14} />
}

const SIDEBAR_LABELS: Record<SidebarSection, string> = {
  get places() {
    return t('常用位置')
  },
  get favorites() {
    return t('收藏')
  },
  get tags() {
    return t('标签')
  },
  get devices() {
    return t('设备')
  },
  get connections() {
    return t('连接')
  },
}

type ReorderKind = 'section' | 'place'
type ReorderBox = { top: number; left: number; width: number; height: number; center: number }
type ReorderDrag = {
  kind: ReorderKind
  from: number
  to: number
  left: number
  top: number
  width: number
  height: number
  gap: number
  settling: boolean
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
  const [drag, setDrag] = useState<ReorderDrag | null>(null)
  const detach = useRef<(() => void) | null>(null)
  const dropTimer = useRef<number | null>(null)
  useEffect(
    () => () => {
      detach.current?.()
      document.body.classList.remove('dragging-reorder')
      if (dropTimer.current !== null) window.clearTimeout(dropTimer.current)
    },
    [],
  )
  const reorder = <T,>(items: T[], from: number, to: number) => {
    const next = [...items]
    const [item] = next.splice(from, 1)
    next.splice(to, 0, item)
    return next
  }
  const beginReorder = (event: React.PointerEvent, kind: ReorderKind, index: number) => {
    if ((event.target as Element).closest('input,button') || drag) return
    const row = event.currentTarget as HTMLElement
    const box = row.getBoundingClientRect()
    const rects: ReorderBox[] = Array.from(
      row.parentElement?.querySelectorAll<HTMLElement>(`[data-reorder="${kind}"]`) ?? [],
    ).map((element) => {
      const rect = element.getBoundingClientRect()
      return {
        top: rect.top,
        left: rect.left,
        width: rect.width,
        height: rect.height,
        center: rect.top + rect.height / 2,
      }
    })
    const gap = rects.length > 1 ? Math.max(0, rects[1].top - (rects[0].top + rects[0].height)) : 0
    const startX = event.clientX
    const startY = event.clientY
    const grab = event.clientY - box.top
    let started = false
    let to = index
    const move = (next: PointerEvent) => {
      if (!started) {
        if (Math.hypot(next.clientX - startX, next.clientY - startY) < 5) return
        started = true
        document.body.classList.add('dragging-reorder')
      }
      next.preventDefault()
      const top = next.clientY - grab
      // Insert after every row whose centre the lifted row has passed. The
      // source row is excluded so the count is the final array index.
      const center = top + box.height / 2
      let nextTo = 0
      for (let i = 0; i < rects.length; i++) {
        if (i !== index && center > rects[i].center) nextTo++
      }
      to = nextTo
      setDrag({
        kind,
        from: index,
        to,
        left: box.left,
        top,
        width: box.width,
        height: box.height,
        gap,
        settling: false,
      })
    }
    const finish = (next: Event) => {
      detach.current?.()
      document.body.classList.remove('dragging-reorder')
      if (!started || next.type === 'pointercancel') {
        setDrag(null)
        return
      }
      const target = to
      setDrag((current) =>
        current ? { ...current, top: rects[target].top, settling: true } : current,
      )
      dropTimer.current = window.setTimeout(() => {
        dropTimer.current = null
        setDrag(null)
        if (target === index) return
        if (kind === 'section') {
          update({ sidebarOrder: reorder(prefs.sidebarOrder, index, target) })
        } else {
          const keys = orderedPlaces.map((location) => location.path || location.name)
          update({ placeOrder: reorder(keys, index, target) })
        }
      }, 160)
    }
    const cleanup = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', finish)
      window.removeEventListener('pointercancel', finish)
      detach.current = null
    }
    detach.current = cleanup
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

  const sectionRow = (id: SidebarSection) => (
    <>
      <GripVertical size={13} className="drag-handle" />
      <span>{SIDEBAR_LABELS[id]}</span>
      <input
        type="checkbox"
        aria-label={t('显示{0}', SIDEBAR_LABELS[id])}
        checked={!prefs.hiddenSections.includes(id)}
        onChange={() =>
          update({
            hiddenSections: prefs.hiddenSections.includes(id)
              ? prefs.hiddenSections.filter((section) => section !== id)
              : [...prefs.hiddenSections, id],
          })
        }
        onPointerDown={(event) => event.stopPropagation()}
      />
    </>
  )
  const placeRow = (location: Location, ghost = false) => {
    const key = location.path || location.name
    const hidden = prefs.hiddenPlaces.includes(key)
    return (
      <>
        <GripVertical size={13} className="drag-handle" />
        <span className="settings-choice-icon place-icon">{placeIcon(location.name)}</span>
        <span className="place-name">{location.name}</span>
        <input
          type="checkbox"
          checked={!hidden}
          aria-label={ghost ? undefined : t('显示{0}', location.name)}
          readOnly={ghost}
          tabIndex={ghost ? -1 : undefined}
          onChange={
            ghost
              ? undefined
              : () =>
                  update({
                    hiddenPlaces: hidden
                      ? prefs.hiddenPlaces.filter((item) => item !== key)
                      : [...prefs.hiddenPlaces, key],
                  })
          }
        />
      </>
    )
  }
  // While a row is lifted the rest of the list slides by the height of the
  // vacated slot, opening a gap under the pointer for the row to land in.
  const shift = (kind: ReorderKind, index: number) => {
    if (!drag || drag.kind !== kind || index === drag.from) return undefined
    const distance = drag.height + drag.gap
    if (drag.from < drag.to && index > drag.from && index <= drag.to)
      return `translate3d(0, -${distance}px, 0)`
    if (drag.to < drag.from && index >= drag.to && index < drag.from)
      return `translate3d(0, ${distance}px, 0)`
    return undefined
  }
  const lifted = drag
  const liftedPlace =
    lifted?.kind === 'place' && orderedPlaces[lifted.from] ? orderedPlaces[lifted.from] : null
  const liftedSection =
    lifted?.kind === 'section' && prefs.sidebarOrder[lifted.from]
      ? prefs.sidebarOrder[lifted.from]
      : null
  return (
    <>
      <div className="settings-card">
        <div className="settings-card-header">
          <span>{t('区域顺序')}</span>
          <button
            className="settings-reset"
            onClick={() =>
              update({
                sidebarOrder: ['places', 'favorites', 'tags', 'devices', 'connections'],
                placeOrder: [],
                hiddenPlaces: [],
                hiddenSections: [],
              })
            }
          >
            <RotateCcw size={11} />
            {t('恢复预设')}
          </button>
        </div>
        <div className={`sidebar-order ${drag?.kind === 'section' ? 'reordering' : ''}`}>
          {prefs.sidebarOrder.map((id, index) => (
            <div
              className={`sidebar-order-row ${drag?.kind === 'section' && drag.from === index ? 'reorder-source' : ''}`}
              key={id}
              data-reorder="section"
              data-reorder-index={index}
              style={{ transform: shift('section', index) }}
              onPointerDown={(event) => beginReorder(event, 'section', index)}
            >
              {sectionRow(id)}
            </div>
          ))}
        </div>
      </div>
      <div className="settings-card">
        <div className="settings-card-header">
          <span>{t('常用位置')}</span>
        </div>
        <div className={`place-order ${drag?.kind === 'place' ? 'reordering' : ''}`}>
          {orderedPlaces.map((location, index) => (
            <div
              className={`place-row ${drag?.kind === 'place' && drag.from === index ? 'reorder-source' : ''}`}
              key={location.path || location.name}
              data-reorder="place"
              data-reorder-index={index}
              style={{ transform: shift('place', index) }}
              onPointerDown={(event) => beginReorder(event, 'place', index)}
            >
              {placeRow(location)}
            </div>
          ))}
        </div>
      </div>
      {lifted &&
        createPortal(
          <div
            className={`reorder-ghost ${lifted.kind === 'section' ? 'sidebar-order-row' : 'place-row'} ${lifted.settling ? 'settling' : ''}`}
            style={{
              left: lifted.left,
              width: lifted.width,
              height: lifted.height,
              transform: `translate3d(0, ${lifted.top}px, 0) scale(1.01)`,
            }}
            aria-hidden="true"
          >
            {liftedSection && sectionRow(liftedSection)}
            {liftedPlace && placeRow(liftedPlace, true)}
          </div>,
          document.body,
        )}
    </>
  )
}

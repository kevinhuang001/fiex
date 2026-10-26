import { t } from '@/i18n'
import { useEffect, useRef, useState, type PointerEvent } from 'react'
import { createPortal } from 'react-dom'
import { SquareTerminal, X } from 'lucide-react'

type TabBox = { left: number; top: number; width: number; height: number; center: number }
type DragOrigin = { pointerX: number; grab: number; rects: TabBox[]; gap: number }
type DragState = {
  from: number
  to: number
  left: number
  top: number
  width: number
  height: number
  gap: number
  settling: boolean
}
export interface TerminalTabInfo {
  id: string
  number: number
  shellName?: string
}

/** Terminal tab strip with the same pointer-driven reorder animation as the
 *  workspace tab bar: lift the tab, slide neighbours, settle, then reorder. */
export function TerminalTabs({
  tabs,
  active,
  onSelect,
  onClose,
  onMove,
}: {
  tabs: TerminalTabInfo[]
  active: string
  onSelect: (id: string) => void
  onClose: (id: string) => void
  onMove: (from: number, to: number) => void
}) {
  const [drag, setDrag] = useState<DragState | null>(null)
  const dragged = useRef(false)
  const origin = useRef<DragOrigin | null>(null)
  const detach = useRef<(() => void) | null>(null)
  const dropTimer = useRef<number | null>(null)
  const strip = useRef<HTMLDivElement>(null)

  useEffect(
    () => () => {
      detach.current?.()
      if (dropTimer.current !== null) window.clearTimeout(dropTimer.current)
    },
    [],
  )

  const beginDrag = (event: PointerEvent, from: number) => {
    if (event.button !== 0 || drag || !strip.current) return
    if ((event.target as Element).closest('.terminal-panel-close')) return
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
    const rects = Array.from(
      strip.current.querySelectorAll<HTMLElement>('.terminal-panel-tab'),
    ).map((element) => {
      const box = element.getBoundingClientRect()
      return {
        left: box.left,
        top: box.top,
        width: box.width,
        height: box.height,
        center: box.left + box.width / 2,
      }
    })
    const gap = rects.length > 1 ? Math.max(0, rects[1].left - (rects[0].left + rects[0].width)) : 3
    origin.current = { pointerX: event.clientX, grab: event.clientX - rect.left, rects, gap }
    dragged.current = false
    let started = false
    let to = from
    const move = (next: globalThis.PointerEvent) => {
      const data = origin.current
      if (!data) return
      if (!started) {
        if (Math.abs(next.clientX - data.pointerX) < 5) return
        started = true
        dragged.current = true
      }
      next.preventDefault()
      const box = data.rects[from]
      let nextTo = 0
      for (let index = 0; index < data.rects.length; index++) {
        if (index !== from && next.clientX > data.rects[index].center) nextTo++
      }
      to = nextTo
      setDrag({
        from,
        to,
        left: next.clientX - data.grab,
        top: box.top,
        width: box.width,
        height: box.height,
        gap: data.gap,
        settling: false,
      })
    }
    const finish = (next: globalThis.PointerEvent) => {
      detach.current?.()
      const data = origin.current
      origin.current = null
      if (!started || next.type === 'pointercancel' || !data) {
        setDrag(null)
        window.setTimeout(() => {
          dragged.current = false
        }, 0)
        return
      }
      const target = to
      setDrag((current) =>
        current ? { ...current, left: data.rects[target].left, settling: true } : current,
      )
      dropTimer.current = window.setTimeout(() => {
        dropTimer.current = null
        setDrag(null)
        if (target !== from) onMove(from, target)
        window.setTimeout(() => {
          dragged.current = false
        }, 0)
      }, 170)
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

  const shift = (index: number) => {
    if (!drag || index === drag.from) return undefined
    const distance = drag.width + drag.gap
    if (drag.from < drag.to && index > drag.from && index <= drag.to)
      return `translate3d(-${distance}px, 0, 0)`
    if (drag.to < drag.from && index >= drag.to && index < drag.from)
      return `translate3d(${distance}px, 0, 0)`
    return undefined
  }
  const lifted = drag ? tabs[drag.from] : undefined
  return (
    <>
      <div
        className={`terminal-panel-tabs ${drag ? 'is-dragging' : ''}`}
        role="tablist"
        aria-label={t('终端标签')}
        ref={strip}
      >
        {tabs.map((tab, index) => (
          <div
            className={`terminal-panel-tab ${tab.id === active ? 'active' : ''} ${drag?.from === index ? 'is-source' : ''}`}
            key={tab.id}
            style={{ transform: shift(index) }}
            onPointerDown={(event) => beginDrag(event, index)}
          >
            <button
              type="button"
              role="tab"
              aria-selected={tab.id === active}
              onClick={() => {
                if (!dragged.current) onSelect(tab.id)
              }}
            >
              <SquareTerminal size={12} />
              {tab.shellName || t('终端')} {tab.number}
            </button>
            <button
              type="button"
              className="terminal-panel-close"
              title={t('关闭终端')}
              aria-label={t('关闭终端 {0}', tab.number)}
              onClick={() => onClose(tab.id)}
            >
              <X size={12} />
            </button>
          </div>
        ))}
      </div>
      {drag && lifted
        ? createPortal(
            <div
              className={`terminal-panel-tab terminal-tab-ghost ${lifted.id === active ? 'active' : ''} ${drag.settling ? 'settling' : ''}`}
              style={{
                width: drag.width,
                height: drag.height,
                transform: `translate3d(${drag.left}px, ${drag.top}px, 0) scale(1.02)`,
              }}
              aria-hidden
            >
              <button type="button" tabIndex={-1}>
                <SquareTerminal size={12} />
                {lifted.shellName || t('终端')} {lifted.number}
              </button>
              <button type="button" tabIndex={-1}>
                <X size={12} />
              </button>
            </div>,
            document.body,
          )
        : null}
    </>
  )
}

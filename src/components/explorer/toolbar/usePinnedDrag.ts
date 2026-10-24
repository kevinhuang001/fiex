import type { AppController } from '@/hooks/useAppController'
import { useEffect, useRef, useState, type PointerEvent } from 'react'

type PinBox = { left: number; top: number; width: number; height: number; center: number }

export type PinDrag = {
  from: number
  to: number
  left: number
  top: number
  width: number
  height: number
  gap: number
  settling: boolean
}

function reorder<T>(items: T[], from: number, to: number) {
  const next = [...items]
  const [moved] = next.splice(from, 1)
  next.splice(to, 0, moved)
  return next
}

type UsePinnedDragOptions = {
  pinnedKeys: string[] | undefined
  update: AppController['update']
}

/** Pointer-following reorder state for the pinned plugin tools. The strip's
 *  buttons are the drag handles: the hook owns the ghost geometry and the
 *  displacement transform, while the caller renders the strip and the ghost. */
export function usePinnedDrag({ pinnedKeys, update }: UsePinnedDragOptions) {
  const [pinDrag, setPinDrag] = useState<PinDrag | null>(null)
  const pinDragged = useRef(false)
  const pinOrigin = useRef<{
    pointerX: number
    grab: number
    rects: PinBox[]
    gap: number
  } | null>(null)
  const pinDetach = useRef<(() => void) | null>(null)
  const pinDropTimer = useRef<number | null>(null)
  const pinStrip = useRef<HTMLDivElement>(null)
  const keys = pinnedKeys || []
  useEffect(
    () => () => {
      pinDetach.current?.()
      if (pinDropTimer.current !== null) window.clearTimeout(pinDropTimer.current)
    },
    [],
  )
  const beginPinDrag = (event: PointerEvent, from: number) => {
    if (event.button !== 0 || pinDrag || !pinStrip.current || !pinnedKeys) return
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
    const rects = Array.from(
      pinStrip.current.querySelectorAll<HTMLElement>('.plugin-tool-button'),
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
    const gap = rects.length > 1 ? Math.max(0, rects[1].left - (rects[0].left + rects[0].width)) : 4
    pinOrigin.current = { pointerX: event.clientX, grab: event.clientX - rect.left, rects, gap }
    pinDragged.current = false
    let started = false
    let to = from
    const move = (next: globalThis.PointerEvent) => {
      const data = pinOrigin.current
      if (!data) return
      if (!started) {
        if (Math.abs(next.clientX - data.pointerX) < 5) return
        started = true
        pinDragged.current = true
      }
      next.preventDefault()
      const box = data.rects[from]
      // Same rule as the tab strip: the pointer decides the slot, so a tool is
      // displaced once half of it is covered instead of a whole button later.
      let nextTo = 0
      for (let index = 0; index < data.rects.length; index++) {
        if (index !== from && next.clientX > data.rects[index].center) nextTo++
      }
      to = nextTo
      setPinDrag({
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
      pinDetach.current?.()
      const data = pinOrigin.current
      pinOrigin.current = null
      if (!started || next.type === 'pointercancel' || !data) {
        setPinDrag(null)
        setTimeout(() => {
          pinDragged.current = false
        }, 0)
        return
      }
      const target = to
      setPinDrag((current) =>
        current ? { ...current, left: data.rects[target].left, settling: true } : current,
      )
      pinDropTimer.current = window.setTimeout(() => {
        pinDropTimer.current = null
        setPinDrag(null)
        if (target !== from) {
          update({ pinnedPluginTools: reorder(keys, from, target) })
        }
        setTimeout(() => {
          pinDragged.current = false
        }, 0)
      }, 160)
    }
    const cleanup = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', finish)
      window.removeEventListener('pointercancel', finish)
      pinDetach.current = null
    }
    pinDetach.current = cleanup
    window.addEventListener('pointermove', move, { passive: false })
    window.addEventListener('pointerup', finish)
    window.addEventListener('pointercancel', finish)
  }
  const pinShift = (index: number) => {
    if (!pinDrag || index === pinDrag.from) return undefined
    const distance = pinDrag.width + pinDrag.gap
    if (pinDrag.from < pinDrag.to && index > pinDrag.from && index <= pinDrag.to)
      return `translate3d(-${distance}px, 0, 0)`
    if (pinDrag.to < pinDrag.from && index >= pinDrag.to && index < pinDrag.from)
      return `translate3d(${distance}px, 0, 0)`
    return undefined
  }
  return { pinStrip, pinDrag, pinDragged, beginPinDrag, pinShift }
}

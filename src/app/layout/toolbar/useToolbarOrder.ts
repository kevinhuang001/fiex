import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'

type ItemBox = {
  left: number
  width: number
  center: number
}

type Drag = {
  from: number
  to: number
  x: number
  width: number
  gap: number
}

function reorder(keys: string[], from: number, to: number) {
  const next = [...keys]
  const [moved] = next.splice(from, 1)
  next.splice(to, 0, moved)
  return next
}

/** Reorders icons inside one toolbar section. Sections never cross the plugin
 *  separators; the caller renders each section's items in the returned order. */
export function useToolbarOrder({
  keys,
  onChange,
}: {
  keys: string[]
  onChange: (keys: string[]) => void
}) {
  const container = useRef<HTMLDivElement>(null)
  const [drag, setDrag] = useState<Drag | null>(null)
  const suppress = useRef(false)
  const detach = useRef<(() => void) | null>(null)
  useEffect(() => () => detach.current?.(), [])
  const begin = (event: ReactPointerEvent, from: number) => {
    if (event.button !== 0 || drag || !container.current) return
    const startX = event.clientX
    const startY = event.clientY
    const items = Array.from(container.current.querySelectorAll<HTMLElement>('[data-toolbar-item]'))
    const boxes: ItemBox[] = items.map((item) => {
      const rect = item.getBoundingClientRect()
      return { left: rect.left, width: rect.width, center: rect.left + rect.width / 2 }
    })
    if (!boxes[from]) return
    const gap = boxes.length > 1 ? Math.max(0, boxes[1].left - (boxes[0].left + boxes[0].width)) : 4
    let active = false
    let to = from
    const move = (next: globalThis.PointerEvent) => {
      if (!active) {
        if (Math.hypot(next.clientX - startX, next.clientY - startY) < 5) return
        active = true
        document.body.classList.add('dragging-toolbar')
      }
      next.preventDefault()
      let nextTo = 0
      for (let index = 0; index < boxes.length; index++) {
        if (index !== from && next.clientX > boxes[index].center) nextTo++
      }
      to = nextTo
      setDrag({ from, to, x: next.clientX - startX, width: boxes[from].width, gap })
    }
    const finish = (next: globalThis.PointerEvent) => {
      detach.current?.()
      detach.current = null
      document.body.classList.remove('dragging-toolbar')
      setDrag(null)
      if (!active || next.type === 'pointercancel') return
      suppress.current = true
      window.setTimeout(() => {
        suppress.current = false
      }, 0)
      if (to !== from) onChange(reorder(keys, from, to))
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
      return `translateX(-${distance}px)`
    if (drag.to < drag.from && index >= drag.to && index < drag.from)
      return `translateX(${distance}px)`
    return undefined
  }
  return {
    container,
    drag,
    begin,
    shift,
    suppressClick: () => suppress.current,
  }
}

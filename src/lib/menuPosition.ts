import { useLayoutEffect, useState, type RefObject } from 'react'

const MARGIN = 8

/** Keeps a fixed-position menu inside the viewport: clamp on the near edges,
 * flip above/left of the pointer when the menu would overflow the far edges. */
function clampMenu(x: number, y: number, width: number, height: number) {
  const maxLeft = window.innerWidth - width - MARGIN
  const maxTop = window.innerHeight - height - MARGIN
  const left = x + width + MARGIN > window.innerWidth ? x - width : x
  const top = y + height + MARGIN > window.innerHeight ? y - height : y
  return {
    left: Math.max(MARGIN, Math.min(left, Math.max(MARGIN, maxLeft))),
    top: Math.max(MARGIN, Math.min(top, Math.max(MARGIN, maxTop))),
  }
}

/** Measures the rendered menu and keeps it viewport-safe as provider content
 * arrives asynchronously. Builtin and plugin items are resolved after the menu
 * opens, so a single measurement at mount would leave a grown menu offscreen. */
export function useMenuPosition(
  x: number,
  y: number,
  ref: RefObject<HTMLElement | null>,
  open = true,
) {
  const [position, setPosition] = useState({ left: x, top: y })
  useLayoutEffect(() => {
    if (!open || !ref.current) return
    const element = ref.current
    const update = () => {
      const rect = element.getBoundingClientRect()
      const next = clampMenu(x, y, rect.width, rect.height)
      setPosition((current) =>
        current.left === next.left && current.top === next.top ? current : next,
      )
    }
    update()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(update)
    observer.observe(element)
    return () => observer.disconnect()
  }, [open, x, y, ref])
  return position
}

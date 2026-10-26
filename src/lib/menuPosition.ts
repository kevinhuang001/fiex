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

/** Measures the rendered menu once and returns a viewport-safe position. */
export function useMenuPosition(
  x: number,
  y: number,
  ref: RefObject<HTMLElement | null>,
  open = true,
) {
  const [position, setPosition] = useState({ left: x, top: y })
  useLayoutEffect(() => {
    if (!open || !ref.current) return
    const rect = ref.current.getBoundingClientRect()
    setPosition(clampMenu(x, y, rect.width, rect.height))
  }, [open, x, y, ref])
  return position
}

import { useLayoutEffect, useState, type RefObject } from 'react'

const MARGIN = 8

export interface MenuPlacement {
  left: number
  top: number
}

export interface MenuViewport {
  width: number
  height: number
}

/** Places a menu at its anchor and keeps the whole menu on screen.
 *
 * The pointer stays the anchor: when the menu does not fit, it moves by the
 * smallest amount that brings it fully inside the viewport instead of flipping
 * to the other side of the pointer. The menu is never capped, so every item is
 * visible and no scrollbar appears. */
export function placeMenu(
  x: number,
  y: number,
  width: number,
  height: number,
  viewport: MenuViewport,
): MenuPlacement {
  const maxLeft = Math.max(MARGIN, viewport.width - width - MARGIN)
  const maxTop = Math.max(MARGIN, viewport.height - height - MARGIN)
  return {
    left: Math.max(MARGIN, Math.min(x, maxLeft)),
    top: Math.max(MARGIN, Math.min(y, maxTop)),
  }
}

/** Measures the rendered menu and keeps it pointer-anchored and fully visible.
 * Builtin and plugin items are resolved after the menu opens, so the menu grows
 * while visible; the effect re-runs on that growth and only shifts the menu as
 * far as the viewport requires. */
export function useMenuPosition(
  x: number,
  y: number,
  ref: RefObject<HTMLElement | null>,
  open = true,
) {
  const [placement, setPlacement] = useState<MenuPlacement>({ left: x, top: y })
  useLayoutEffect(() => {
    if (!open || !ref.current) return
    const element = ref.current
    const update = () => {
      const rect = element.getBoundingClientRect()
      const next = placeMenu(x, y, Math.ceil(rect.width), element.scrollHeight, {
        width: window.innerWidth,
        height: window.innerHeight,
      })
      setPlacement((current) =>
        current.left === next.left && current.top === next.top ? current : next,
      )
    }
    update()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(update)
    observer.observe(element)
    return () => observer.disconnect()
  }, [open, x, y, ref])
  return placement
}

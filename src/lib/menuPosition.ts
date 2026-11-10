import { useLayoutEffect, useState, type RefObject } from 'react'

const MARGIN = 8
const SUBMENU_GAP = 7

export interface MenuPlacement {
  left: number
  top: number
}

export interface MenuViewport {
  width: number
  height: number
}

export function placeSubmenu(
  anchor: { left: number; right: number; top: number; bottom: number },
  size: { width: number; height: number },
  viewport: MenuViewport,
  preferredSide: 'left' | 'right',
): { left: boolean; up: boolean; maxWidth: number } {
  const roomLeft = Math.max(0, Math.floor(anchor.left - MARGIN - SUBMENU_GAP))
  const roomRight = Math.max(0, Math.floor(viewport.width - anchor.right - MARGIN - SUBMENU_GAP))
  const preferLeft = preferredSide === 'left'
  const preferredRoom = preferLeft ? roomLeft : roomRight
  const otherRoom = preferLeft ? roomRight : roomLeft
  const left =
    preferredRoom >= size.width
      ? preferLeft
      : otherRoom >= size.width
        ? !preferLeft
        : roomLeft >= roomRight
  const upRoom = anchor.bottom - MARGIN
  const downRoom = viewport.height - anchor.top - MARGIN
  return {
    left,
    up: downRoom < size.height && upRoom > downRoom,
    maxWidth: Math.min(420, Math.max(1, left ? roomLeft : roomRight)),
  }
}

export function measureSubmenu(
  anchor: HTMLElement,
  panel: HTMLElement,
  preferredSide: 'left' | 'right',
) {
  const anchorRect = anchor.getBoundingClientRect()
  const panelRect = panel.getBoundingClientRect()
  return placeSubmenu(
    anchorRect,
    { width: panelRect.width, height: panelRect.height },
    { width: window.innerWidth, height: window.innerHeight },
    preferredSide,
  )
}

/** Places a menu at its anchor and keeps the whole menu on screen.
 * Flip toward the opposite side of the pointer when there is room, then clamp
 * to the viewport margin for tight layouts. */
export function placeMenu(
  x: number,
  y: number,
  width: number,
  height: number,
  viewport: MenuViewport,
): MenuPlacement {
  const maxLeft = Math.max(MARGIN, viewport.width - width - MARGIN)
  const maxTop = Math.max(MARGIN, viewport.height - height - MARGIN)
  const left =
    x + width > viewport.width - MARGIN && x - width >= MARGIN ? x - width : Math.min(x, maxLeft)
  const top =
    y + height > viewport.height - MARGIN && y - height >= MARGIN ? y - height : Math.min(y, maxTop)
  return {
    left: Math.max(MARGIN, left),
    top: Math.max(MARGIN, top),
  }
}

/** Measures the rendered menu and keeps it pointer-anchored and fully visible.
 * Core rows are resolved before this menu is shown, and plugin rows live in a
 * lazy submenu, so the top-level height stays stable while it is open. */
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
      // Measure the top-level box, not scrollHeight: an absolutely positioned
      // (lazy) submenu may overflow it and must not move the menu.
      const next = placeMenu(x, y, Math.ceil(rect.width), Math.ceil(rect.height), {
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

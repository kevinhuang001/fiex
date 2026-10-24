import { useEffect, useRef, useState } from 'react'
import { useMenuPosition } from '@/lib/menuPosition'
import type { SidebarMenuState } from './types'

/** Position, ref and dismissal for the sidebar's trash/favorite context menu. */
export function useSidebarMenu() {
  const [menu, setMenu] = useState<SidebarMenuState | null>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const menuPosition = useMenuPosition(menu?.x ?? 0, menu?.y ?? 0, menuRef, Boolean(menu))
  useEffect(() => {
    if (!menu) return
    // pointerdown fires before click, so an item can only be activated when the
    // press inside the menu is ignored by the outside-click handler.
    const close = (event: Event) => {
      if (menuRef.current?.contains(event.target as Node)) return
      setMenu(null)
    }
    window.addEventListener('pointerdown', close)
    window.addEventListener('blur', close)
    return () => {
      window.removeEventListener('pointerdown', close)
      window.removeEventListener('blur', close)
    }
  }, [menu])
  return { menu, setMenu, menuRef, menuPosition }
}

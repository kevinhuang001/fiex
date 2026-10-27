import { ChevronRight } from 'lucide-react'
import { useEffect, useRef, type Dispatch, type SetStateAction } from 'react'
import type { ContextSubmenu, MenuPosition, SetMenu } from './types'

/** A menu row that expands into a nested menu on hover or click. */
export function SubmenuItem({
  item,
  open,
  setOpen,
  menuPosition,
  setMenu,
}: {
  item: ContextSubmenu
  open: boolean
  setOpen: Dispatch<SetStateAction<boolean>>
  menuPosition: MenuPosition
  setMenu: SetMenu
}) {
  const timer = useRef<number | null>(null)
  const cancel = () => {
    if (timer.current !== null) window.clearTimeout(timer.current)
    timer.current = null
  }
  useEffect(() => cancel, [])
  const openNow = () => {
    cancel()
    setOpen(true)
  }
  const scheduleClose = () => {
    cancel()
    timer.current = window.setTimeout(() => setOpen(false), 500)
  }
  return (
    <div className="context-submenu-wrap" onMouseEnter={openNow} onMouseLeave={scheduleClose}>
      <button
        role="menuitem"
        disabled={item.disabled}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={(event) => {
          event.stopPropagation()
          cancel()
          setOpen((value) => !value)
        }}
      >
        {item.icon}
        {item.label}
        <ChevronRight className="submenu-arrow" size={13} />
      </button>
      {open && !item.disabled && (
        <div
          className={`context-submenu${menuPosition.left + 380 > window.innerWidth ? ' open-left' : ''}`}
          role="menu"
          onMouseEnter={cancel}
        >
          {item.submenu.map((entry) => (
            <button
              role="menuitem"
              key={entry.label}
              onClick={() => {
                entry.action()
                setMenu(null)
              }}
            >
              {entry.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

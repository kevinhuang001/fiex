import { ChevronRight } from 'lucide-react'
import type { Dispatch, SetStateAction } from 'react'
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
  return (
    <div
      className="context-submenu-wrap"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
    >
      <button
        role="menuitem"
        disabled={item.disabled}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={(event) => {
          event.stopPropagation()
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

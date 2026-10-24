import type { ContextAction, SetMenu } from './types'

/** One plain menu row with its optional shortcut hint. */
export function ContextMenuButton({ item, setMenu }: { item: ContextAction; setMenu: SetMenu }) {
  return (
    <button
      role="menuitem"
      disabled={item.disabled}
      className={item.danger ? 'context-danger' : undefined}
      onClick={() => {
        item.action()
        setMenu(null)
      }}
    >
      {item.icon}
      {item.label}
      {item.hint && <span className="shortcut">{item.hint}</span>}
    </button>
  )
}

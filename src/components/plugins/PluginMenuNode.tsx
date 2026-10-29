import { t } from '@/i18n'
import {
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from 'react'
import { ChevronRight } from 'lucide-react'

export interface PluginMenuItem {
  id: string
  title: string
  kind: 'action' | 'submenu' | 'separator'
  parentId?: string
}

export interface PluginMenu {
  pluginId: string
  menuSessionId: string
  items: PluginMenuItem[]
}

/** Sibling submenus share one open id so moving to the next level closes the
 *  previous panel immediately instead of leaving two panels side by side. */
export const SubmenuGroupContext = createContext<{
  openId: string | null
  setOpenId: Dispatch<SetStateAction<string | null>>
} | null>(null)

/** Validates the untrusted tree a plugin returns before rendering it. */
export function validPluginMenuItems(items: unknown): items is PluginMenuItem[] {
  if (!Array.isArray(items) || items.length > 100) return false
  const ids = new Set<string>()
  for (const item of items) {
    if (!item || typeof item !== 'object') return false
    const node = item as Record<string, unknown>
    if (
      typeof node.id !== 'string' ||
      !/^[\w.-]{1,80}$/.test(node.id) ||
      ids.has(node.id) ||
      typeof node.title !== 'string' ||
      node.title.length > 128 ||
      !['action', 'submenu', 'separator'].includes(String(node.kind)) ||
      ('parentId' in node && typeof node.parentId !== 'string')
    ) {
      return false
    }
    ids.add(node.id)
  }
  const byId = new Map(items.map((item) => [item.id, item as PluginMenuItem]))
  for (const item of items as PluginMenuItem[]) {
    let depth = 1
    let current = item
    const seen = new Set<string>([item.id])
    while (current.parentId) {
      const parent = byId.get(current.parentId)
      if (!parent || parent.kind !== 'submenu' || seen.has(parent.id) || ++depth > 4) return false
      seen.add(parent.id)
      current = parent
    }
  }
  return true
}

/** One row of a plugin menu. Submenus expand on hover or click and may lazily
 *  ask the plugin for their children the first time they open. */
export function PluginMenuNode({
  item,
  items,
  parentLeft = false,
  preferLeft = false,
  onAction,
  onExpand,
}: {
  item: PluginMenuItem
  items: PluginMenuItem[]
  /** The parent panel already grows left; all descendants must follow. */
  parentLeft?: boolean
  /** The root menu sits on the left half; prefer left when there is room. */
  preferLeft?: boolean
  onAction: (item: PluginMenuItem, anchor?: { x: number; y: number }) => void
  onExpand: (submenuId: string) => void
}) {
  const group = useContext(SubmenuGroupContext)
  const [localExpanded, setLocalExpanded] = useState(false)
  const [childOpen, setChildOpen] = useState<string | null>(null)
  const expanded = group ? group.openId === item.id : localExpanded
  const [openUp, setOpenUp] = useState(false)
  const [openLeft, setOpenLeft] = useState(parentLeft)
  const submenu = useRef<HTMLDivElement>(null)
  const timer = useRef<number | null>(null)
  const cancel = () => {
    if (timer.current !== null) window.clearTimeout(timer.current)
    timer.current = null
  }
  const setExpanded = (value: boolean) => {
    if (group)
      group.setOpenId((current) => (value ? item.id : current === item.id ? null : current))
    else setLocalExpanded(value)
  }
  // Give the pointer time to cross the gap between a submenu trigger and its
  // panel before collapsing the panel.
  const scheduleClose = () => {
    cancel()
    timer.current = window.setTimeout(() => {
      if (group) group.setOpenId((current) => (current === item.id ? null : current))
      else setLocalExpanded(false)
    }, 500)
  }
  useEffect(() => cancel, [])
  useEffect(() => {
    if (!expanded) setChildOpen(null)
  }, [expanded])
  useLayoutEffect(() => {
    if (expanded && submenu.current) {
      const rect = submenu.current.getBoundingClientRect()
      setOpenUp(rect.bottom > window.innerHeight - 8)
      const roomLeft = rect.left - rect.width >= 8
      setOpenLeft(parentLeft || (preferLeft && roomLeft) || rect.right > window.innerWidth - 8)
    }
  }, [expanded, parentLeft, preferLeft])
  if (item.kind === 'separator')
    return (
      <div
        role="separator"
        className="context-divider"
        onMouseEnter={() => group?.setOpenId(null)}
      />
    )
  if (item.kind === 'submenu') {
    const descendants = items.filter((candidate) => candidate.parentId === item.id)
    const expand = () => {
      cancel()
      setExpanded(true)
      if (!descendants.length) onExpand(item.id)
    }
    return (
      <div className="context-submenu-wrap" onMouseEnter={expand} onMouseLeave={scheduleClose}>
        <button
          role="menuitem"
          title={item.title}
          aria-haspopup="menu"
          aria-expanded={expanded}
          onClick={(event) => {
            event.stopPropagation()
            cancel()
            if (!expanded) expand()
            else setExpanded(false)
          }}
        >
          <span className="context-submenu-label">{item.title}</span>
          <ChevronRight size={13} className="submenu-arrow" />
        </button>
        {expanded && (
          <div
            ref={submenu}
            className={
              'context-submenu' + (openUp ? ' open-up' : '') + (openLeft ? ' open-left' : '')
            }
            role="menu"
            aria-label={item.title}
            onMouseEnter={cancel}
          >
            {descendants.length ? (
              <SubmenuGroupContext.Provider value={{ openId: childOpen, setOpenId: setChildOpen }}>
                {descendants.map((entry) => (
                  <PluginMenuNode
                    key={entry.id}
                    item={entry}
                    items={items}
                    parentLeft={openLeft}
                    preferLeft={preferLeft}
                    onAction={onAction}
                    onExpand={onExpand}
                  />
                ))}
              </SubmenuGroupContext.Provider>
            ) : (
              <div className="context-submenu-empty">{t('暂无可用操作')}</div>
            )}
          </div>
        )}
      </div>
    )
  }
  return (
    <button
      role="menuitem"
      title={item.title}
      onMouseEnter={() => group?.setOpenId(null)}
      onClick={(event) => onAction(item, { x: event.clientX, y: event.clientY })}
    >
      <span className="context-submenu-label">{item.title}</span>
    </button>
  )
}

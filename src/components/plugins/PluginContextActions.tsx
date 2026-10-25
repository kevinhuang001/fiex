import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { ChevronRight, Puzzle } from 'lucide-react'
import { call } from '@/lib/api'
import { usePluginCatalog } from '@/lib/plugins'
import { beginActivity, finishActivity, updateActivity } from '@/hooks/useActivity'
import { OPEN_PLUGIN_PANEL, PLUGIN_RESULT_EVENT } from '@/components/plugins/PluginPanelHost'

interface MenuItem {
  id: string
  title: string
  kind: 'action' | 'submenu' | 'separator'
  parentId?: string
}
interface Menu {
  pluginId: string
  menuSessionId: string
  items: MenuItem[]
}
interface Context {
  paths: string[]
  path: string
  isDirectory: boolean
  readOnly: boolean
  isRemote: boolean
}

function validMenu(items: unknown): items is MenuItem[] {
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
  const byId = new Map(items.map((item) => [item.id, item as MenuItem]))
  for (const item of items as MenuItem[]) {
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

function MenuNode({
  item,
  items,
  onAction,
}: {
  item: MenuItem
  items: MenuItem[]
  onAction: (item: MenuItem, anchor?: { x: number; y: number }) => void
}) {
  const [expanded, setExpanded] = useState(false)
  const [openUp, setOpenUp] = useState(false)
  const submenu = useRef<HTMLDivElement>(null)
  const timer = useRef<number | null>(null)
  const cancel = () => {
    if (timer.current !== null) window.clearTimeout(timer.current)
    timer.current = null
  }
  useEffect(() => cancel, [])
  useLayoutEffect(() => {
    if (expanded && submenu.current) {
      setOpenUp(submenu.current.getBoundingClientRect().bottom > window.innerHeight - 8)
    }
  }, [expanded])
  if (item.kind === 'separator') return <div role="separator" className="context-divider" />
  if (item.kind === 'submenu') {
    const descendants = items.filter((candidate) => candidate.parentId === item.id)
    if (!descendants.length) return null
    return (
      <div
        className="context-submenu-wrap"
        onMouseEnter={() => {
          cancel()
          setExpanded(true)
        }}
        onMouseLeave={() => {
          cancel()
          timer.current = window.setTimeout(() => setExpanded(false), 180)
        }}
      >
        <button
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={expanded}
          onClick={(event) => {
            event.stopPropagation()
            cancel()
            setExpanded((value) => !value)
          }}
        >
          <Puzzle size={15} />
          {item.title}
          <ChevronRight size={13} className="submenu-arrow" />
        </button>
        {expanded && (
          <div
            ref={submenu}
            className={'context-submenu' + (openUp ? ' open-up' : '')}
            role="menu"
            onMouseEnter={cancel}
          >
            {descendants.map((entry) => (
              <MenuNode key={entry.id} item={entry} items={items} onAction={onAction} />
            ))}
          </div>
        )}
      </div>
    )
  }
  return (
    <button
      role="menuitem"
      onClick={(event) => onAction(item, { x: event.clientX, y: event.clientY })}
    >
      <Puzzle size={15} />
      {item.title}
    </button>
  )
}

export function PluginContextActions({
  paths,
  path,
  isDirectory,
  readOnly,
  isRemote,
  onClose,
  onError,
}: Context & { onClose: () => void; onError: (error: string) => void }) {
  const plugins = usePluginCatalog()
  const [menus, setMenus] = useState<Menu[]>([])
  const selectionKey = JSON.stringify(paths)
  useEffect(() => {
    let active = true
    const enabled = plugins.filter((plugin) => {
      if (!plugin.enabled) return false
      const contribution = plugin.extensions?.contextMenus
      if (contribution === true) return true
      return contribution !== null && typeof contribution === 'object'
    })
    void Promise.all(
      enabled.map(async (plugin) => {
        try {
          const response = await call<{ menuSessionId: string; items: unknown }>(
            'plugin_context_menu_resolve',
            {
              pluginId: plugin.id,
              directory: path,
              paths: JSON.parse(selectionKey) as string[],
              isDirectory,
              readOnly,
              isRemote,
            },
          )
          return validMenu(response.items)
            ? { pluginId: plugin.id, menuSessionId: response.menuSessionId, items: response.items }
            : null
        } catch {
          return null
        }
      }),
    ).then((results) => {
      if (active) setMenus(results.filter((result): result is Menu => result !== null))
    })
    return () => {
      active = false
    }
  }, [plugins, selectionKey, isDirectory, readOnly, isRemote])

  const invoke = async (menu: Menu, item: MenuItem, anchor?: { x: number; y: number }) => {
    onClose()
    const pluginId = menu.pluginId
    const plugin = plugins.find((entry) => entry.id === pluginId)
    let activityId = ''
    try {
      // No content permission is handed out during resolve; executing a real
      // click enters the operation with its exact selected paths.
      const response = await call<unknown>('plugin_context_menu_execute', {
        menuSessionId: menu.menuSessionId,
        itemId: item.id,
      })
      if (
        response &&
        typeof response === 'object' &&
        'kind' in response &&
        response.kind === 'panel' &&
        'assetPath' in response &&
        typeof response.assetPath === 'string'
      ) {
        const entry = response as {
          assetPath: string
          panelId?: string
          title?: string
          size?: { width: number; height: number }
        }
        window.dispatchEvent(
          new CustomEvent(OPEN_PLUGIN_PANEL, {
            detail: {
              pluginId,
              panelId: entry.panelId || item.id,
              title: entry.title || item.title,
              icon: plugin?.icon,
              selection: paths,
              directory: path,
              anchor,
              source: 'contextMenu',
              size: entry.size,
              assetPath: entry.assetPath,
            },
          }),
        )
        return
      }
      activityId = beginActivity(
        (plugin?.name || '插件') + ' · ' + item.title,
        paths.length + ' 个项目',
        undefined,
        'plugin',
      )
      updateActivity(activityId, { indeterminate: true })
      finishActivity(activityId, undefined, '已提交')
      if (response && typeof response === 'object') {
        window.dispatchEvent(
          new CustomEvent(PLUGIN_RESULT_EVENT, { detail: { title: item.title, result: response } }),
        )
      }
    } catch (error) {
      if (activityId) finishActivity(activityId, String(error))
      onError(String(error))
    }
  }
  return (
    <>
      {menus.flatMap((menu) =>
        menu.items
          .filter((item) => !item.parentId)
          .map((item) => (
            <MenuNode
              key={menu.pluginId + ':' + item.id}
              item={item}
              items={menu.items}
              onAction={(selected, anchor) => void invoke(menu, selected, anchor)}
            />
          )),
      )}
    </>
  )
}

import { t } from '@/i18n'
import { Pencil, Pin, Plus, Puzzle, RefreshCw, Server, TerminalSquare, Trash2 } from 'lucide-react'
import { Fragment, useEffect, useRef, useState } from 'react'
import { call } from '@/lib/api'
import { useMenuPosition } from '@/lib/menuPosition'
import { MenuLayer } from '@/components/ui/MenuLayer'
import { beginActivity, finishActivity, updateActivity } from '@/hooks/useActivity'
import type { RemoteConnection } from '@/lib/types'
import type { SidebarProps } from './types'

type Destination = { kind: 'vfs'; path: string } | { kind: 'pluginView' }

interface ConnectionMenuItem {
  id: string
  kind: 'action' | 'separator'
  title: string
  icon?: string
  danger?: boolean
  disabled?: boolean
  placement?: 'tail'
}

interface ConnectionMenu {
  kind: 'builtin' | 'plugin'
  providerId: string
  pluginId?: string
  pluginName?: string
  menuSessionId: string
  items: ConnectionMenuItem[]
}

const menuIcons = {
  pencil: Pencil,
  pin: Pin,
  sync: RefreshCw,
  trash: Trash2,
} as const

/** Remote connections and saved SSH sessions, with add/session menus. */
const isPluginViewConnection = (connection: RemoteConnection) =>
  connection.mountPath.startsWith('fiex-plugin-view://')

/** A connection is open when a folder tab targets its mount, or when its
 *  terminal/plugin view is the tab's target. Open sessions stay in the sidebar,
 *  so the add menu only offers the saved sessions that are not open yet. */
const isOpen = (connection: RemoteConnection, tabs: string[]) => {
  const mount = connection.mountPath.replace(/\/$/, '')
  return tabs.some(
    (path) =>
      path === `fiex-plugin-view://${connection.id}` ||
      (mount && (path === mount || path.startsWith(mount + '/'))),
  )
}
export function Connections({
  x,
  onAddConnection,
  onEditConnection,
  onRemoveConnection,
  notify,
}: Pick<
  SidebarProps,
  'x' | 'onAddConnection' | 'onEditConnection' | 'onRemoveConnection' | 'notify'
>) {
  const [plusOpen, setPlusOpen] = useState(false)
  const [plusAnchor, setPlusAnchor] = useState({ x: 8, y: 0 })
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null)
  const [menus, setMenus] = useState<ConnectionMenu[]>([])
  const [menuLoading, setMenuLoading] = useState(false)
  const plus = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const plusMenu = useRef<HTMLDivElement>(null)
  const plusPosition = useMenuPosition(plusAnchor.x, plusAnchor.y, plusMenu, plusOpen)
  const contextPosition = useMenuPosition(menu?.x ?? 0, menu?.y ?? 0, menuRef, Boolean(menu))
  const saved = x.connections.filter(
    (connection) => connection.saved !== false && !isOpen(connection, x.tabs),
  )

  useEffect(() => {
    if (!plusOpen && !menu) return
    const close = (event: MouseEvent) => {
      if (
        plusOpen &&
        !plus.current?.contains(event.target as Node) &&
        !plusMenu.current?.contains(event.target as Node)
      )
        setPlusOpen(false)
      if (menu && !menuRef.current?.contains(event.target as Node)) setMenu(null)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [plusOpen, menu])

  useEffect(() => {
    if (!menu) {
      setMenus([])
      setMenuLoading(false)
      return
    }
    let active = true
    setMenus([])
    setMenuLoading(true)
    void call<ConnectionMenu[]>('plugin_connection_menu_resolve', { connectionId: menu.id })
      .then((value) => {
        if (active) setMenus(value)
      })
      .catch((error) => {
        if (active) x.setError(String(error))
      })
      .finally(() => {
        if (active) setMenuLoading(false)
      })
    return () => {
      active = false
    }
    // Menu coordinates only decide where the panel renders; the resolved list
    // belongs to the connection id.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [menu?.id])

  const open = (connection: RemoteConnection) => {
    setPlusOpen(false)
    setMenu(null)
    void call<Destination>('connection_open', { id: connection.id })
      .then((target) =>
        x.navigate(target.kind === 'vfs' ? target.path : 'fiex-plugin-view://' + connection.id),
      )
      .catch((error) => x.setError(String(error)))
  }
  const runMenuItem = async (source: ConnectionMenu, item: ConnectionMenuItem) => {
    const connection = menu ? x.connections.find((value) => value.id === menu.id) : undefined
    setMenu(null)
    if (!connection || item.kind === 'separator' || item.disabled) return
    let activityId = ''
    try {
      if (item.id === 'connection.syncNow') {
        activityId = beginActivity(
          `${connection.name} · ${item.title}`,
          undefined,
          undefined,
          'sync',
          connection.id,
        )
        updateActivity(activityId, { indeterminate: true })
      }
      const result = await call<{ kind?: string; command?: string }>(
        'plugin_connection_menu_execute',
        {
          connectionId: connection.id,
          providerId: source.providerId,
          itemId: item.id,
          menuSessionId: source.menuSessionId,
        },
      )
      if (result?.kind === 'coreAction') {
        if (result.command === 'editConnection') onEditConnection(connection)
        if (result.command === 'closeConnection') onRemoveConnection(connection.id)
      } else {
        await x.refreshConnections()
        if (item.id === 'connection.syncNow') x.reload()
        if (item.id === 'connection.saveSession') notify(t('已保存 Session：{0}', connection.name))
        if (item.id === 'connection.toggleAutostart')
          notify(connection.autostart ? t('已取消自启动') : t('已设置为自启动'))
      }
      if (activityId) finishActivity(activityId)
    } catch (error) {
      if (activityId) finishActivity(activityId, String(error))
      x.setError(String(error))
    }
  }
  const current = menu ? x.connections.find((connection) => connection.id === menu.id) : undefined
  const builtinMenus = menus.filter((value) => value.kind === 'builtin')
  const pluginMenus = menus.filter((value) => value.kind === 'plugin')
  const renderItem = (source: ConnectionMenu, item: ConnectionMenuItem) => {
    if (item.kind === 'separator')
      return (
        <div key={`${source.providerId}:${item.id}`} role="separator" className="context-divider" />
      )
    const Icon = source.kind === 'plugin' ? Puzzle : menuIcons[item.icon as keyof typeof menuIcons]
    return (
      <button
        key={`${source.providerId}:${item.id}`}
        type="button"
        role="menuitem"
        disabled={item.disabled}
        className={item.danger ? 'danger' : undefined}
        onClick={() => void runMenuItem(source, item)}
      >
        {Icon && <Icon size={14} />}
        {item.title}
      </button>
    )
  }

  return (
    <Fragment>
      <div className="sidebar-section-title connection-section-title">
        <span className="sidebar-label">{t('连接')}</span>
        <div className="connection-add" ref={plus}>
          <button
            className="icon-button"
            title={t('添加连接或打开 Session')}
            aria-label={t('添加连接')}
            aria-expanded={plusOpen}
            onClick={() => {
              if (!plusOpen && plus.current) {
                const rect = plus.current.getBoundingClientRect()
                setPlusAnchor({ x: rect.left, y: rect.bottom + 6 })
              }
              setPlusOpen((value) => !value)
            }}
          >
            <Plus size={14} />
          </button>
          {plusOpen && (
            <MenuLayer>
              <div
                ref={plusMenu}
                className="connection-session-menu plus"
                role="menu"
                style={plusPosition}
              >
                {saved.length > 0 ? (
                  saved.map((connection) => (
                    <button
                      type="button"
                      role="menuitem"
                      key={connection.id}
                      onClick={() => open(connection)}
                    >
                      {isPluginViewConnection(connection) ? (
                        <TerminalSquare size={14} />
                      ) : (
                        <Server size={14} />
                      )}
                      <span>
                        <strong>{connection.name}</strong>
                      </span>
                    </button>
                  ))
                ) : (
                  <p className="sidebar-hint">{t('还没有保存的 Session')}</p>
                )}
                <button
                  type="button"
                  role="menuitem"
                  className="connection-session-new"
                  onClick={() => {
                    setPlusOpen(false)
                    onAddConnection()
                  }}
                >
                  <Plus size={14} />
                  <span>{t('新建连接…')}</span>
                </button>
              </div>
            </MenuLayer>
          )}
        </div>
      </div>
      {x.connections.length === 0 ? (
        <p className="sidebar-hint">{t('点击 + 添加连接')}</p>
      ) : (
        x.connections.map((connection) => (
          <div
            className="device-row"
            key={connection.id}
            onContextMenu={(event) => {
              event.preventDefault()
              setMenu({ x: event.clientX, y: event.clientY, id: connection.id })
            }}
          >
            <button
              className={`nav-item connection-nav ${x.path === connection.mountPath || x.path.startsWith(connection.mountPath.replace(/\/$/, '') + '/') ? 'active' : ''}`}
              onClick={() => open(connection)}
            >
              {isPluginViewConnection(connection) ? <TerminalSquare /> : <Server />}
              <span>
                <span>{connection.name}</span>
              </span>
              {connection.autostart && (
                <span
                  className="connection-autostart"
                  title={t('开机自启动')}
                  aria-label={t('自启动')}
                >
                  <Pin size={10} />
                </span>
              )}
            </button>
          </div>
        ))
      )}
      {menu && current && (
        <MenuLayer>
          <div
            ref={menuRef}
            className="connection-session-menu floating"
            role="menu"
            style={contextPosition}
          >
            {menuLoading && !menus.length && <p className="sidebar-hint">{t('正在加载菜单…')}</p>}
            {builtinMenus.flatMap((source) =>
              source.items
                .filter((item) => item.placement !== 'tail')
                .map((item) => renderItem(source, item)),
            )}
            {pluginMenus.length > 0 && <div role="separator" className="context-divider" />}
            {pluginMenus.flatMap((source) => source.items.map((item) => renderItem(source, item)))}
            {builtinMenus.flatMap((source) =>
              source.items
                .filter((item) => item.placement === 'tail')
                .map((item) => renderItem(source, item)),
            )}
          </div>
        </MenuLayer>
      )}
    </Fragment>
  )
}

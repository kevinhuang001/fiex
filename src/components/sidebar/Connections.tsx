import { Pin, Plus, Server, TerminalSquare } from 'lucide-react'
import { Fragment, useEffect, useRef, useState } from 'react'
import { call } from '@/lib/api'
import { useMenuPosition } from '@/lib/menuPosition'
import type { RemoteConnection } from '@/lib/types'
import type { SidebarProps } from './types'

type Destination =
  | { kind: 'vfs'; path: string }
  | { kind: 'pluginView' }
  | { kind: 'terminal'; connectionId: string }

/** Remote connections and saved SSH sessions, with add/session menus. */
export function Connections({
  x,
  onAddConnection,
  onRemoveConnection,
  notify,
}: Pick<SidebarProps, 'x' | 'onAddConnection' | 'onRemoveConnection' | 'notify'>) {
  const [plusOpen, setPlusOpen] = useState(false)
  const [plusAnchor, setPlusAnchor] = useState({ x: 8, y: 0 })
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null)
  const plus = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const plusMenu = useRef<HTMLDivElement>(null)
  const plusPosition = useMenuPosition(plusAnchor.x, plusAnchor.y, plusMenu, plusOpen)
  const contextPosition = useMenuPosition(menu?.x ?? 0, menu?.y ?? 0, menuRef, Boolean(menu))
  const saved = x.connections.filter((connection) => connection.saved !== false)

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

  const open = (connection: RemoteConnection) => {
    setPlusOpen(false)
    setMenu(null)
    void call<Destination>('connection_open', { id: connection.id })
      .then((target) =>
        x.navigate(
          target.kind === 'vfs'
            ? target.path
            : target.kind === 'pluginView'
              ? 'fiex-plugin-view://' + connection.id
              : 'fiex-terminal://' + target.connectionId,
        ),
      )
      .catch((error) => x.setError(String(error)))
  }
  const save = async (connection: RemoteConnection) => {
    setMenu(null)
    try {
      await call('update_connection_session', { id: connection.id, saved: true })
      await x.refreshConnections()
      notify(`已保存 Session：${connection.name}`)
    } catch (error) {
      x.setError(String(error))
    }
  }
  const toggleAutostart = async (connection: RemoteConnection) => {
    setMenu(null)
    try {
      await call('update_connection_session', {
        id: connection.id,
        autostart: !connection.autostart,
      })
      await x.refreshConnections()
      notify(connection.autostart ? '已取消自启动' : '已设置为自启动')
    } catch (error) {
      x.setError(String(error))
    }
  }
  const current = menu ? x.connections.find((connection) => connection.id === menu.id) : undefined

  return (
    <Fragment>
      <div className="sidebar-section-title">
        <span className="sidebar-label">连接</span>
        <div className="connection-add" ref={plus}>
          <button
            className="icon-button"
            title="添加连接或打开 Session"
            aria-label="添加连接"
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
                    {connection.kind === 'ssh' ? (
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
                <p className="sidebar-hint">还没有保存的 Session</p>
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
                <span>新建连接…</span>
              </button>
            </div>
          )}
        </div>
      </div>
      {x.connections.length === 0 ? (
        <p className="sidebar-hint">点击 + 添加连接</p>
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
              {connection.kind === 'ssh' ? <TerminalSquare /> : <Server />}
              <span>
                <span>{connection.name}</span>
                <small>{connection.kind}</small>
              </span>
              {connection.autostart && (
                <span className="connection-autostart" title="开机自启动" aria-label="自启动">
                  <Pin size={10} />
                </span>
              )}
            </button>
          </div>
        ))
      )}
      {menu && current && (
        <div
          ref={menuRef}
          className="connection-session-menu floating"
          role="menu"
          style={contextPosition}
        >
          <button type="button" role="menuitem" onClick={() => void save(current)}>
            保存 Session
          </button>
          <button type="button" role="menuitem" onClick={() => void toggleAutostart(current)}>
            {current.autostart ? '取消自启动' : '设置为自启动'}
          </button>
          <button
            type="button"
            role="menuitem"
            className="danger"
            onClick={() => {
              setMenu(null)
              onRemoveConnection(current.id)
            }}
          >
            关闭连接
          </button>
        </div>
      )}
    </Fragment>
  )
}

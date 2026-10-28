import { t } from '@/i18n'
import { Pin, Plus, Server } from 'lucide-react'
import { Fragment, useEffect, useRef, useState } from 'react'
import { call } from '@/lib/api'
import { useMenuPosition } from '@/lib/menuPosition'
import type { RemoteConnection } from '@/lib/types'
import type { SidebarProps } from './types'

type Destination = { kind: 'vfs'; path: string } | { kind: 'pluginView' }

/** Saved connections navigate through their owning provider. */
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
        x.navigate(target.kind === 'vfs' ? target.path : 'fiex-plugin-view://' + connection.id),
      )
      .catch((error) => x.setError(String(error)))
  }
  const save = async (connection: RemoteConnection) => {
    setMenu(null)
    try {
      await call('update_connection_session', { id: connection.id, saved: true })
      await x.refreshConnections()
      notify(t('已保存 Session：{0}', connection.name))
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
      notify(connection.autostart ? t('已取消自启动') : t('已设置为自启动'))
    } catch (error) {
      x.setError(String(error))
    }
  }
  const current = menu ? x.connections.find((connection) => connection.id === menu.id) : undefined

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
                    <Server size={14} />
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
              <Server />
              <span>
                <span>{connection.name}</span>
                <small>{connection.pluginId}</small>
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
        <div
          ref={menuRef}
          className="connection-session-menu floating"
          role="menu"
          style={contextPosition}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setMenu(null)
              onEditConnection(current)
            }}
          >
            {t('编辑连接…')}
          </button>
          <button type="button" role="menuitem" onClick={() => void save(current)}>
            {t('保存 Session')}
          </button>
          <button type="button" role="menuitem" onClick={() => void toggleAutostart(current)}>
            {current.autostart ? t('取消自启动') : t('设置为自启动')}
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
            {t('关闭连接')}
          </button>
        </div>
      )}
    </Fragment>
  )
}

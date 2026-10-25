import { Plus, Server, X } from 'lucide-react'
import { Fragment } from 'react'
import { call } from '@/lib/api'
import type { SidebarProps } from './types'

/** Remote connections, with the add button and per-connection disconnect. */
export function Connections({
  x,
  onAddConnection,
  onRemoveConnection,
}: Pick<SidebarProps, 'x' | 'onAddConnection' | 'onRemoveConnection'>) {
  return (
    <Fragment>
      <div className="sidebar-section-title">
        <span className="sidebar-label">连接</span>
        <button
          className="icon-button"
          title="添加连接"
          aria-label="添加连接"
          onClick={onAddConnection}
        >
          <Plus size={14} />
        </button>
      </div>
      {x.connections.length === 0 ? (
        <p className="sidebar-hint">点击 + 添加连接</p>
      ) : (
        x.connections.map((connection) => (
          <div className="device-row" key={connection.id}>
            <button
              className={`nav-item ${x.path === connection.mountPath || x.path.startsWith(connection.mountPath.replace(/\/$/, '') + '/') ? 'active' : ''}`}
              onClick={() => {
                void call<
                  | { kind: 'vfs'; path: string }
                  | { kind: 'pluginView' }
                  | { kind: 'terminal'; connectionId: string }
                >('connection_open', { id: connection.id })
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
              }}
            >
              <Server />
              <span>
                <span>{connection.name}</span>
                <small>{connection.kind}</small>
              </span>
            </button>
            <button
              className="device-eject icon-button"
              title="关闭连接"
              onClick={() => onRemoveConnection(connection.id)}
            >
              <X size={13} />
            </button>
          </div>
        ))
      )}
    </Fragment>
  )
}

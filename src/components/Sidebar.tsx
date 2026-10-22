import { commonPlaces } from '@/lib/places'
import {
  Home,
  Monitor,
  Download,
  Image,
  Music,
  Film,
  FileText,
  Folder,
  Plus,
  Trash,
  HardDrive,
  X,
  Trash2,
  AppWindow,
  Clock,
  Server,
  Unplug,
  Usb,
} from 'lucide-react'
import { Fragment, useEffect, useRef, useState } from 'react'
import { size } from '@/lib/files'
import type { Explorer } from '@/hooks/useExplorer'
import type { Preferences } from '@/lib/types'
import { fileVisual } from '@/lib/fileIcons'
import { tagHex } from '@/lib/tags'
interface Props {
  x: Explorer
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
  onTrash: () => void
  onEmptyTrash: () => void
  onApplications: () => void
  onAddConnection: () => void
  onRemoveConnection: (id: string) => void
  onEject: (path: string) => void
  onEjectDevice: (device: string) => void
  onMount: (device: string) => void
  trashPath: string | null
}
const DISK_COLORS = ['#5b9cf8', '#55b98a', '#e0a44a', '#b47ad6', '#e06c75', '#56b6c9']

export function Sidebar({
  x,
  prefs,
  update,
  onTrash,
  onEmptyTrash,
  onApplications,
  onAddConnection,
  onRemoveConnection,
  onEject,
  onEjectDevice,
  onMount,
  trashPath,
}: Props) {
  const allTags = prefs.tags
  const [menu, setMenu] = useState<
    | { kind: 'trash'; x: number; y: number }
    | { kind: 'favorite'; x: number; y: number; path: string }
    | null
  >(null)
  const menuRef = useRef<HTMLDivElement>(null)
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
  const iconFor = (name: string) =>
    name === '废纸篓' ? (
      <Trash2 />
    ) : name === '最近使用' ? (
      <Clock />
    ) : name === '个人文件夹' ? (
      <Home />
    ) : name === '桌面' ? (
      <Monitor />
    ) : name === '下载' ? (
      <Download />
    ) : name === '图片' ? (
      <Image />
    ) : name === '音乐' ? (
      <Music />
    ) : name === '视频' ? (
      <Film />
    ) : name === '文稿' ? (
      <FileText />
    ) : name === '应用程序' ? (
      <AppWindow />
    ) : (
      <Folder />
    )

  return (
    <aside className="sidebar">
      <div className="sidebar-title" data-tauri-drag-region />
      <div className="sidebar-content">
        {prefs.sidebarOrder.map((section) => {
          if (section === 'places') {
            const order = new Map(prefs.placeOrder.map((key, index) => [key, index]))
            const places = commonPlaces(x.locations)
              .filter((location) => !prefs.hiddenPlaces.includes(location.path || location.name))
              .sort(
                (left, right) =>
                  (order.get(left.path || left.name) ?? Number.MAX_SAFE_INTEGER) -
                  (order.get(right.path || right.name) ?? Number.MAX_SAFE_INTEGER),
              )
            if (!places.length) return null
            return (
              <Fragment key="places">
                <div className="sidebar-label">常用位置</div>
                {places.map((location) => (
                  <button
                    className={`nav-item ${(location.kind === 'trash' ? trashPath && x.path === trashPath : location.path && x.path === location.path) ? 'active' : ''}`}
                    key={`${location.kind}:${location.path || location.name}`}
                    onClick={() =>
                      location.kind === 'trash'
                        ? onTrash()
                        : location.kind === 'applications' && !location.path
                          ? onApplications()
                          : x.navigate(location.path)
                    }
                    onContextMenu={(event) => {
                      if (location.kind !== 'trash') return
                      event.preventDefault()
                      setMenu({ kind: 'trash', x: event.clientX, y: event.clientY })
                    }}
                    data-drop-folder={
                      location.kind === 'trash' || location.kind === 'recent'
                        ? undefined
                        : location.path || undefined
                    }
                  >
                    {iconFor(location.name)}
                    <span>{location.name}</span>
                  </button>
                ))}
              </Fragment>
            )
          }
          if (section === 'favorites') {
            return (
              <div className="favorites-section" data-bookmark-drop="true" key="favorites">
                <div className="sidebar-section-title">
                  <span className="sidebar-label">收藏</span>
                </div>
                {prefs.favorites.length ? (
                  prefs.favorites.map((location) => (
                    <div
                      className="favorite-row"
                      key={location.path}
                      data-drop-folder={location.path}
                    >
                      <button
                        className={`nav-item ${x.path === location.path ? 'active' : ''}`}
                        onClick={() => x.navigate(location.path)}
                        onContextMenu={(event) => {
                          event.preventDefault()
                          setMenu({
                            kind: 'favorite',
                            x: event.clientX,
                            y: event.clientY,
                            path: location.path,
                          })
                        }}
                      >
                        <FavoriteIcon name={location.name} />
                        <span>{location.name}</span>
                      </button>
                    </div>
                  ))
                ) : (
                  <p className="sidebar-hint">将文件夹拖到这里收藏</p>
                )}
              </div>
            )
          }
          if (section === 'tags') {
            return (
              <Fragment key="tags">
                <div className="sidebar-section-title">
                  <span className="sidebar-label">标签</span>
                </div>
                {allTags.map((tag) => (
                  <button
                    key={tag.name}
                    className={`nav-item tag-nav ${x.tagFilter === tag.name ? 'active' : ''}`}
                    onClick={() => x.setTagFilter(x.tagFilter === tag.name ? null : tag.name)}
                  >
                    <span className="tag-dot" style={{ background: tagHex(tag) }} />
                    <span>{tag.name}</span>
                  </button>
                ))}
              </Fragment>
            )
          }
          if (section === 'connections') {
            return (
              <Fragment key="connections">
                <div className="sidebar-section-title">
                  <span className="sidebar-label">连接</span>
                  <button
                    className="icon-button"
                    title="添加 WebDAV 或 SMB 连接"
                    aria-label="添加 WebDAV 或 SMB 连接"
                    onClick={onAddConnection}
                  >
                    <Plus size={14} />
                  </button>
                </div>
                {x.connections.length === 0 ? (
                  <p className="sidebar-hint">WebDAV · SMB</p>
                ) : (
                  x.connections.map((connection) => (
                    <div className="device-row" key={connection.id}>
                      <button
                        className={`nav-item ${x.path === connection.mountPath || x.path.startsWith(connection.mountPath + '/') ? 'active' : ''}`}
                        onClick={() => x.navigate(connection.mountPath)}
                      >
                        <Server />
                        <span>
                          <span>{connection.name}</span>
                          <small>{connection.kind === 'webdav' ? 'WebDAV' : 'SMB'}</small>
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
          return (
            <Fragment key="devices">
              <div className="sidebar-section-title devices-title">
                <span className="sidebar-label">设备</span>
              </div>
              {x.locations
                .filter((location) => location.kind === 'disk')
                .map((location, index) => {
                  const color = DISK_COLORS[index % DISK_COLORS.length]
                  const used =
                    location.total > 0
                      ? Math.min(100, Math.max(0, (1 - location.available / location.total) * 100))
                      : 0
                  return (
                    <div className="device-row" key={location.path || location.device}>
                      <button
                        className={`nav-item disk-item ${location.mounted || location.usbfs ? '' : 'disk-unmounted'} ${x.path === location.path ? 'active' : ''}`}
                        onClick={() =>
                          location.path ? x.navigate(location.path) : onMount(location.device)
                        }
                        title={location.path ? location.path : '未挂载，点击授权挂载'}
                      >
                        {location.removable ? <Usb color={color} /> : <HardDrive color={color} />}
                        <span>
                          <span>{location.name || location.path}</span>
                          <small
                            className="disk-capacity"
                            title={`可用 ${size(location.available)} / 总计 ${size(location.total)}`}
                          >
                            {!location.mounted
                              ? size(location.total)
                              : location.total > 0
                                ? `${size(location.available)} / ${size(location.total)}`
                                : size(location.available)}
                            {location.filesystem && (
                              <span className="disk-fs">
                                {location.filesystem.toUpperCase()}
                                {location.usbfs
                                  ? ['APFS', 'HFS', 'HFS+', 'HFSX', 'BTRFS'].includes(
                                      location.filesystem.toUpperCase(),
                                    )
                                    ? ' · 只读'
                                    : ' · 内置'
                                  : !location.mounted
                                    ? ' · 挂载'
                                    : ''}
                              </span>
                            )}
                          </small>
                          {location.total > 0 && (
                            <span
                              className="disk-meter"
                              role="progressbar"
                              aria-label={`${location.name || location.path} 已用 ${Math.round(used)}%`}
                              aria-valuenow={Math.round(used)}
                              aria-valuemin={0}
                              aria-valuemax={100}
                            >
                              <i style={{ width: `${used}%`, background: color }} />
                            </span>
                          )}
                        </span>
                      </button>
                      {location.usbfs ? (
                        <button
                          className="device-eject icon-button"
                          title="弹出 U 盘"
                          onClick={() => onEjectDevice(location.device)}
                        >
                          <Unplug size={13} />
                        </button>
                      ) : (
                        location.mounted &&
                        location.path !== '/' &&
                        !/^[A-Za-z]:[\\/]?$/.test(location.path) && (
                          <button
                            className="device-eject icon-button"
                            title="弹出设备"
                            onClick={() => onEject(location.path)}
                          >
                            <Unplug size={13} />
                          </button>
                        )
                      )}
                    </div>
                  )
                })}
            </Fragment>
          )
        })}
      </div>
      {menu && (
        <div
          ref={menuRef}
          className="context-menu sidebar-menu"
          role="menu"
          aria-label={menu.kind === 'trash' ? '废纸篓' : '收藏'}
          style={{ left: menu.x, top: menu.y }}
        >
          {menu.kind === 'trash' ? (
            <>
              <button
                role="menuitem"
                onClick={() => {
                  setMenu(null)
                  onTrash()
                }}
              >
                <Folder size={14} />
                打开废纸篓
              </button>
              <button
                role="menuitem"
                onClick={() => {
                  setMenu(null)
                  onEmptyTrash()
                }}
              >
                <Trash size={14} />
                清空废纸篓
              </button>
            </>
          ) : (
            <button
              role="menuitem"
              onClick={() => {
                update({
                  favorites: prefs.favorites.filter((item) => item.path !== menu.path),
                })
                setMenu(null)
              }}
            >
              <X size={14} />
              从侧栏移除
            </button>
          )}
        </div>
      )}
    </aside>
  )
}

function FavoriteIcon({ name }: { name: string }) {
  const visual = fileVisual({
    name,
    isDir: true,
    extension: name.split('.').pop()?.toLowerCase() || '',
  })
  return (
    <span className="file-icon" style={{ width: 18, height: 18 }}>
      <img
        src={visual.url}
        className={visual.generic ? 'generic-folder' : ''}
        alt=""
        draggable={false}
      />
    </span>
  )
}

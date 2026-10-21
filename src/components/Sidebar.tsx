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
  HardDrive,
  X,
  Trash2,
  AppWindow,
  Server,
  Unplug,
  Usb,
} from 'lucide-react'
import { Fragment } from 'react'
import { size } from '@/lib/files'
import type { Explorer } from '@/hooks/useExplorer'
import type { Preferences } from '@/lib/types'
import { fileVisual } from '@/lib/fileIcons'
import { TAG_PRESETS } from '@/lib/tags'
interface Props {
  x: Explorer
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
  favorite: () => void
  control: (action: 'close' | 'minimize' | 'toggleMaximize') => void
  onTrash: () => void
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
  favorite,
  control,
  onTrash,
  onApplications,
  onAddConnection,
  onRemoveConnection,
  onEject,
  onEjectDevice,
  onMount,
  trashPath,
}: Props) {
  const iconFor = (name: string) =>
    name === '个人文件夹' ? (
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
      <div className="sidebar-title" data-tauri-drag-region>
        <div className="traffic">
          <button title="关闭窗口" className="traffic-close" onClick={() => control('close')} />
          <button title="最小化" className="traffic-minimize" onClick={() => control('minimize')} />
          <button
            title="最大化"
            className="traffic-maximize"
            onClick={() => control('toggleMaximize')}
          />
        </div>
        <span data-tauri-drag-region>Fiex</span>
      </div>
      <div className="sidebar-content">
        {prefs.sidebarOrder.map((section) => {
          if (section === 'places') {
            const order = new Map(prefs.placeOrder.map((key, index) => [key, index]))
            const places = x.locations
              .filter(
                (location) =>
                  (location.kind === 'favorite' || location.kind === 'applications') &&
                  !prefs.hiddenPlaces.includes(location.path || location.name),
              )
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
                    className={`nav-item ${location.path && x.path === location.path ? 'active' : ''}`}
                    key={`${location.kind}:${location.path || location.name}`}
                    onClick={() =>
                      location.kind === 'applications' && !location.path
                        ? onApplications()
                        : x.navigate(location.path)
                    }
                    data-drop-folder={location.path || undefined}
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
                  <button className="icon-button" title="收藏当前文件夹" onClick={favorite}>
                    <Plus size={14} />
                  </button>
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
                      >
                        <FavoriteIcon name={location.name} />
                        <span>{location.name}</span>
                      </button>
                      <button
                        className="favorite-remove icon-button"
                        title="移除收藏"
                        onClick={() =>
                          update({
                            favorites: prefs.favorites.filter(
                              (item) => item.path !== location.path,
                            ),
                          })
                        }
                      >
                        <X size={12} />
                      </button>
                    </div>
                  ))
                ) : (
                  <></>
                )}
              </div>
            )
          }
          if (section === 'tags') {
            return (
              <Fragment key="tags">
                <div className="sidebar-label tags-label">标签</div>
                {TAG_PRESETS.map((tag) => (
                  <button
                    key={tag.name}
                    className={`nav-item tag-nav ${x.tagFilter === tag.name ? 'active' : ''}`}
                    onClick={() => x.setTagFilter(x.tagFilter === tag.name ? null : tag.name)}
                  >
                    <span className="tag-dot" style={{ background: tag.hex }} />
                    <span>{tag.name}</span>
                  </button>
                ))}
              </Fragment>
            )
          }
          return (
            <Fragment key="devices">
              <div className="sidebar-section-title devices-title">
                <span className="sidebar-label">设备</span>
                <button
                  className="icon-button"
                  title="添加 WebDAV 或 SMB 连接"
                  onClick={onAddConnection}
                >
                  <Plus size={14} />
                </button>
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
              {x.connections.map((connection) => (
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
              ))}
            </Fragment>
          )
        })}
      </div>
      <div className="sidebar-bottom">
        <button
          className={`nav-item ${trashPath && x.path === trashPath ? 'active' : ''}`}
          onClick={onTrash}
        >
          <Trash2 />
          <span>废纸篓</span>
        </button>
      </div>
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

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
  Star,
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
            const places = x.locations.filter(
              (location) =>
                (location.kind === 'favorite' || location.kind === 'applications') &&
                !prefs.hiddenPlaces.includes(location.path || location.name),
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
                  <button
                    className="icon-button"
                    title="收藏当前文件夹；也可以把文件夹拖到这里"
                    onClick={favorite}
                  >
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
                        <Star />
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
                  <p className="sidebar-hint">将常用文件夹拖到这里收藏</p>
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
                .map((location, index) => (
                  <div className="device-row" key={location.path || location.device}>
                    <button
                      className={`nav-item disk-item ${location.mounted || location.usbfs ? '' : 'disk-unmounted'} ${x.path === location.path ? 'active' : ''}`}
                      onClick={() =>
                        location.path ? x.navigate(location.path) : onMount(location.device)
                      }
                      title={location.path ? location.path : '未挂载，点击授权挂载'}
                    >
                      {location.removable ? <Usb /> : <HardDrive />}
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
                          <i className="disk-meter">
                            <i
                              style={{
                                width: `${(1 - location.available / location.total) * 100}%`,
                                background: DISK_COLORS[index % DISK_COLORS.length],
                              }}
                            />
                          </i>
                        )}
                      </span>
                    </button>
                    {location.mounted &&
                      location.path !== '/' &&
                      !/^[A-Za-z]:[\\/]?$/.test(location.path) && (
                        <button
                          className="device-eject icon-button"
                          title="弹出设备"
                          onClick={() => onEject(location.path)}
                        >
                          <Unplug size={13} />
                        </button>
                      )}
                  </div>
                ))}
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

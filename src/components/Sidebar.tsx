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
} from 'lucide-react'
import { basename, parent, size } from '@/lib/files'
import { gitState } from '@/lib/git'
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
  trashPath: string | null
}
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
        <div className="sidebar-label">常用位置</div>
        {x.locations
          .filter((l) => l.kind === 'favorite' || l.kind === 'applications')
          .map((l) => (
            <button
              className={`nav-item ${l.path && x.path === l.path ? 'active' : ''}`}
              key={`${l.kind}:${l.path || l.name}`}
              onClick={() =>
                l.kind === 'applications' && !l.path ? onApplications() : x.navigate(l.path)
              }
              data-drop-folder={l.path || undefined}
            >
              {iconFor(l.name)}
              <span>{l.name}</span>
            </button>
          ))}
        <div className="favorites-section" data-bookmark-drop="true">
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
            prefs.favorites.map((l) => (
              <div className="favorite-row" key={l.path} data-drop-folder={l.path}>
                <button
                  className={`nav-item ${x.path === l.path ? 'active' : ''}`}
                  onClick={() => x.navigate(l.path)}
                >
                  <Star />
                  <span>{l.name}</span>
                </button>
                <button
                  className="favorite-remove icon-button"
                  title="移除收藏"
                  onClick={() =>
                    update({ favorites: prefs.favorites.filter((f) => f.path !== l.path) })
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
        {x.git && Object.keys(x.git.entries).length > 0 && (
          <>
            <div className="sidebar-label git-label">
              <span>Git</span>
              <small>{x.git.branch}</small>
            </div>
            <div className="git-changes">
              {Object.entries(x.git.entries)
                .sort(([a], [b]) => a.localeCompare(b))
                .map(([path, code]) => {
                  const state = gitState(code)
                  if (!state) return null
                  return (
                    <button
                      key={path}
                      className="git-change"
                      title={path}
                      onClick={() => x.navigate(parent(path))}
                    >
                      <span className={`git-badge git-${state.letter.toLowerCase()}`}>
                        {state.letter}
                      </span>
                      <span className={`git-file git-file-${state.letter.toLowerCase()}`}>
                        {basename(path)}
                      </span>
                      <small>{state.label}</small>
                    </button>
                  )
                })}
            </div>
          </>
        )}
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
        <div className="sidebar-section-title devices-title">
          <span className="sidebar-label">设备</span>
          <button className="icon-button" title="添加 WebDAV 或 SMB 连接" onClick={onAddConnection}>
            <Plus size={14} />
          </button>
        </div>
        {x.locations
          .filter((l) => l.kind === 'disk')
          .map((l) => (
            <div className="device-row" key={l.path}>
              <button
                className={`nav-item disk-item ${x.path === l.path ? 'active' : ''}`}
                onClick={() => x.navigate(l.path)}
              >
                <HardDrive />
                <span>
                  <span>{l.name || l.path}</span>
                  <small>{size(l.available)} 可用</small>
                  {l.total > 0 && (
                    <i>
                      <i style={{ width: `${(1 - l.available / l.total) * 100}%` }} />
                    </i>
                  )}
                </span>
              </button>
              {l.path !== '/' && !/^[A-Za-z]:[\\/]?$/.test(l.path) && (
                <button
                  className="device-eject icon-button"
                  title="弹出设备"
                  onClick={() => onEject(l.path)}
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

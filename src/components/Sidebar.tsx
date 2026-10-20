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
  Settings2,
  ExternalLink,
} from 'lucide-react'
import { size } from '@/lib/files'
import { shortcutLabel } from '@/lib/shortcuts'
import type { Explorer } from '@/hooks/useExplorer'
import type { Preferences } from '@/lib/types'
interface Props {
  x: Explorer
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
  favorite: () => void
  transfer: (paths: string[], dest: string, moving: boolean) => void
  control: (action: 'close' | 'minimize' | 'toggleMaximize') => void
  onSettings: () => void
  onTrash: () => void
}
export function Sidebar({
  x,
  prefs,
  update,
  favorite,
  transfer,
  control,
  onSettings,
  onTrash,
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
          .filter((l) => l.kind === 'favorite')
          .map((l) => (
            <button
              className={`nav-item ${x.path === l.path ? 'active' : ''}`}
              key={l.path}
              onClick={() => x.navigate(l.path)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => {
                e.preventDefault()
                try {
                  transfer(
                    JSON.parse(e.dataTransfer.getData('application/x-fiex')),
                    l.path,
                    e.shiftKey,
                  )
                } catch {}
              }}
            >
              {iconFor(l.name)}
              <span>{l.name}</span>
            </button>
          ))}
        <div className="sidebar-section-title">
          <span className="sidebar-label">收藏</span>
          <button className="icon-button" title="收藏当前文件夹" onClick={favorite}>
            <Plus size={14} />
          </button>
        </div>
        {prefs.favorites.length ? (
          prefs.favorites.map((l) => (
            <div className="favorite-row" key={l.path}>
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
          <p className="sidebar-hint">将常用文件夹留在这里</p>
        )}
        <div className="sidebar-label devices-label">设备</div>
        {x.locations
          .filter((l) => l.kind === 'disk')
          .map((l) => (
            <button
              className={`nav-item disk-item ${x.path === l.path ? 'active' : ''}`}
              key={l.path}
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
          ))}
      </div>
      <div className="sidebar-bottom">
        <button className="nav-item" onClick={onTrash}>
          <Trash2 />
          <span>废纸篓</span>
          <ExternalLink size={12} />
        </button>
        <button className="nav-item" aria-label="设置" onClick={onSettings}>
          <Settings2 />
          <span>设置</span>
          <small>{shortcutLabel(prefs.shortcuts, 'settings', prefs.shortcutStyle)}</small>
        </button>
      </div>
    </aside>
  )
}

import { Folder, HardDrive, Server } from 'lucide-react'
import type { Location, RemoteConnection } from '@/lib/types'

export function PickerSidebar({
  directory,
  places,
  favorites,
  connections,
  disks,
  navigate,
}: {
  directory: string
  places: Location[]
  favorites: Location[]
  connections: RemoteConnection[]
  disks: Location[]
  navigate(path: string): void
}) {
  return (
    <aside className="file-picker-sidebar">
      {places.length > 0 && <p className="file-picker-section">常用位置</p>}
      {places.map((item) => (
        <button
          key={`place:${item.kind}:${item.path || item.name}`}
          type="button"
          className={`file-picker-place ${directory === item.path ? 'active' : ''}`}
          onClick={() => navigate(item.path)}
        >
          <Folder size={14} />
          <span>{item.name}</span>
        </button>
      ))}
      <p className="file-picker-section">收藏</p>
      {!favorites.length && <p className="file-picker-empty">还没有收藏</p>}
      {favorites.map((item) => (
        <button
          key={`favorite:${item.path}`}
          type="button"
          className={`file-picker-place ${directory === item.path ? 'active' : ''}`}
          onClick={() => navigate(item.path)}
        >
          <Folder size={14} />
          <span>{item.name}</span>
        </button>
      ))}
      <p className="file-picker-section">连接</p>
      {!connections.length && <p className="file-picker-empty">没有连接</p>}
      {connections.map((item) => (
        <button
          key={`connection:${item.id}`}
          type="button"
          className={`file-picker-place ${directory === item.mountPath ? 'active' : ''}`}
          onClick={() => navigate(item.mountPath)}
        >
          <Server size={14} />
          <span>{item.name}</span>
        </button>
      ))}
      <p className="file-picker-section">设备</p>
      {!disks.length && <p className="file-picker-empty">没有设备</p>}
      {disks.map((item) => (
        <button
          key={`disk:${item.path || item.device}`}
          type="button"
          className={`file-picker-place ${directory === item.path ? 'active' : ''}`}
          onClick={() => item.path && navigate(item.path)}
        >
          <HardDrive size={14} />
          <span>{item.name}</span>
        </button>
      ))}
    </aside>
  )
}

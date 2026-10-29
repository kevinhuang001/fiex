import { t } from '@/i18n'
import { useEffect, useState } from 'react'
import { stateSnapshot, loadState, saveState } from '@/lib/stateStore'
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
  const [collapsed, setCollapsed] = useState<string[]>(
    () => stateSnapshot<string[]>('picker.sidebarCollapsed') ?? [],
  )
  useEffect(() => {
    let active = true
    void loadState<string[]>('picker.sidebarCollapsed')
      .then((saved) => {
        if (active && Array.isArray(saved)) setCollapsed(saved)
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [])
  const toggle = (section: string) => {
    setCollapsed((previous) => {
      const next = previous.includes(section)
        ? previous.filter((item) => item !== section)
        : [...previous, section]
      saveState('picker.sidebarCollapsed', next)
      return next
    })
  }
  const heading = (section: string, label: string) => (
    <button
      type="button"
      className="file-picker-section"
      aria-expanded={!collapsed.includes(section)}
      onClick={() => toggle(section)}
    >
      {t(label)}
    </button>
  )
  return (
    <aside className="file-picker-sidebar">
      {places.length > 0 && heading('places', '常用位置')}
      {!collapsed.includes('places') &&
        places.map((item) => (
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
      {heading('favorites', '收藏')}
      {!favorites.length && <p className="file-picker-empty">{t('还没有收藏')}</p>}
      {!collapsed.includes('favorites') &&
        favorites.map((item) => (
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
      {heading('connections', '连接')}
      {!connections.length && <p className="file-picker-empty">{t('没有连接')}</p>}
      {!collapsed.includes('connections') &&
        connections.map((item) => (
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
      {heading('disks', '设备')}
      {!disks.length && <p className="file-picker-empty">{t('没有设备')}</p>}
      {!collapsed.includes('disks') &&
        disks.map((item) => (
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

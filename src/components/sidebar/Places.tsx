import { commonPlaces } from '@/lib/places'
import {
  AppWindow,
  Clock,
  Download,
  FileText,
  Film,
  Folder,
  Home,
  Image,
  Monitor,
  Music,
  Trash2,
} from 'lucide-react'
import { Fragment } from 'react'
import type { SidebarProps, SetSidebarMenu } from './types'

function iconFor(name: string) {
  return name === '废纸篓' ? (
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
}

/** The "常用位置" group: trash, recent, home and the standard user folders. */
export function Places({
  x,
  prefs,
  trashPath,
  onTrash,
  onApplications,
  setMenu,
}: Pick<SidebarProps, 'x' | 'prefs' | 'trashPath' | 'onTrash' | 'onApplications'> & {
  setMenu: SetSidebarMenu
}) {
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
    <Fragment>
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

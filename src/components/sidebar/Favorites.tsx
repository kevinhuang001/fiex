import { t } from '@/i18n'
import { fileVisual } from '@/lib/fileIcons'
import type { SidebarProps, SetSidebarMenu } from './types'

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

/** The bookmarked folders, which are also a drop target for new favorites. */
export function Favorites({
  x,
  prefs,
  setMenu,
}: Pick<SidebarProps, 'x' | 'prefs'> & { setMenu: SetSidebarMenu }) {
  return (
    <div className="favorites-section" data-bookmark-drop="true">
      <div className="sidebar-section-title">
        <span className="sidebar-label">{t('收藏')}</span>
      </div>
      {prefs.favorites.length ? (
        prefs.favorites.map((location) => (
          <div className="favorite-row" key={location.path} data-drop-folder={location.path}>
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
        <p className="sidebar-hint">{t('将文件夹拖到这里收藏')}</p>
      )}
    </div>
  )
}

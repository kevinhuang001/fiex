import { t } from '@/i18n'
import { Folder, Trash, X } from 'lucide-react'
import type { RefObject } from 'react'
import type { MenuPosition, SidebarMenuState, SidebarProps, SetSidebarMenu } from './types'

/** The trash and favorite context menu shared by the sidebar sections. */
export function SidebarMenu({
  menu,
  prefs,
  update,
  onTrash,
  onEmptyTrash,
  setMenu,
  menuRef,
  menuPosition,
}: Pick<SidebarProps, 'prefs' | 'update' | 'onTrash' | 'onEmptyTrash'> & {
  menu: SidebarMenuState | null
  setMenu: SetSidebarMenu
  menuRef: RefObject<HTMLDivElement | null>
  menuPosition: MenuPosition
}) {
  if (!menu) return null
  return (
    <div
      ref={menuRef}
      className="context-menu sidebar-menu"
      role="menu"
      aria-label={menu.kind === 'trash' ? t('废纸篓') : t('收藏')}
      style={menuPosition}
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
            {t('打开废纸篓')}
          </button>
          <button
            role="menuitem"
            onClick={() => {
              setMenu(null)
              onEmptyTrash()
            }}
          >
            <Trash size={14} />
            {t('清空废纸篓')}
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
          {t('从侧栏移除')}
        </button>
      )}
    </div>
  )
}

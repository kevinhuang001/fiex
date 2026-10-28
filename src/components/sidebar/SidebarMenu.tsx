import { t } from '@/i18n'
import { isWindows } from '@/lib/platform'
import { Folder, Plug, Trash, Unplug, X } from 'lucide-react'
import type { RefObject } from 'react'
import { MenuLayer } from '@/components/ui/MenuLayer'
import type { MenuPosition, SidebarMenuState, SidebarProps, SetSidebarMenu } from './types'

/** The trash, favorite and device context menus shared by the sidebar sections. */
export function SidebarMenu({
  menu,
  prefs,
  update,
  onTrash,
  onEmptyTrash,
  onEject,
  onEjectDevice,
  onMount,
  setMenu,
  menuRef,
  menuPosition,
}: Pick<
  SidebarProps,
  'prefs' | 'update' | 'onTrash' | 'onEmptyTrash' | 'onEject' | 'onEjectDevice' | 'onMount'
> & {
  menu: SidebarMenuState | null
  setMenu: SetSidebarMenu
  menuRef: RefObject<HTMLDivElement | null>
  menuPosition: MenuPosition
}) {
  if (!menu) return null
  return (
    <MenuLayer>
      <div
        ref={menuRef}
        className="context-menu sidebar-menu"
        role="menu"
        aria-label={
          menu.kind === 'trash'
            ? t(isWindows() ? '回收站' : '废纸篓')
            : menu.kind === 'favorite'
              ? t('收藏')
              : menu.name || t('设备')
        }
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
              {t(isWindows() ? '打开回收站' : '打开废纸篓')}
            </button>
            <button
              role="menuitem"
              onClick={() => {
                setMenu(null)
                onEmptyTrash()
              }}
            >
              <Trash size={14} />
              {t(isWindows() ? '清空回收站' : '清空废纸篓')}
            </button>
          </>
        ) : menu.kind === 'favorite' ? (
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
        ) : (
          <>
            {!menu.mounted && (
              <button
                role="menuitem"
                onClick={() => {
                  setMenu(null)
                  onMount(menu.device)
                }}
              >
                <Plug size={14} />
                {t('挂载')}
              </button>
            )}
            {menu.canEject && (
              <button
                role="menuitem"
                onClick={() => {
                  setMenu(null)
                  if (menu.usbfs) onEjectDevice(menu.device)
                  else if (menu.path) onEject(menu.path)
                }}
              >
                <Unplug size={14} />
                {t('弹出')}
              </button>
            )}
          </>
        )}
      </div>
    </MenuLayer>
  )
}

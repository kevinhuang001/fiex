import { Connections } from './Connections'
import { Devices } from './Devices'
import { Favorites } from './Favorites'
import { Places } from './Places'
import { SidebarMenu } from './SidebarMenu'
import { Tags } from './Tags'
import { useSidebarMenu } from './useSidebarMenu'
import type { SidebarProps } from './types'

export function Sidebar({
  x,
  prefs,
  update,
  onTrash,
  onApplications,
  onAddConnection,
  onEditConnection,
  onRemoveConnection,
  notify,
  onEject,
  onEjectDevice,
  onMount,
  trashPath,
}: SidebarProps) {
  const { menu, setMenu, menuRef, menuPosition } = useSidebarMenu()
  return (
    <aside className="sidebar">
      <div className="sidebar-title" data-tauri-drag-region />
      <div className="sidebar-content">
        {prefs.sidebarOrder.map((section) => {
          if (prefs.hiddenSections.includes(section)) return null
          if (section === 'places') {
            return (
              <Places
                key="places"
                x={x}
                prefs={prefs}
                trashPath={trashPath}
                onTrash={onTrash}
                onApplications={onApplications}
                setMenu={setMenu}
              />
            )
          }
          if (section === 'favorites') {
            return <Favorites key="favorites" x={x} prefs={prefs} setMenu={setMenu} />
          }
          if (section === 'tags') {
            return <Tags key="tags" x={x} prefs={prefs} />
          }
          if (section === 'connections') {
            return (
              <Connections
                key="connections"
                x={x}
                onAddConnection={onAddConnection}
                onEditConnection={onEditConnection}
                onRemoveConnection={onRemoveConnection}
                notify={notify}
              />
            )
          }
          return <Devices key="devices" x={x} onMount={onMount} setMenu={setMenu} />
        })}
      </div>
      <SidebarMenu
        menu={menu}
        prefs={prefs}
        update={update}
        onTrash={onTrash}
        onEject={onEject}
        onEjectDevice={onEjectDevice}
        onMount={onMount}
        setMenu={setMenu}
        menuRef={menuRef}
        menuPosition={menuPosition}
      />
    </aside>
  )
}

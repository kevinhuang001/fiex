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
  onEmptyTrash,
  onApplications,
  onAddConnection,
  onRemoveConnection,
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
            return prefs.showTags ? <Tags key="tags" x={x} prefs={prefs} /> : null
          }
          if (section === 'connections') {
            return (
              <Connections
                key="connections"
                x={x}
                onAddConnection={onAddConnection}
                onRemoveConnection={onRemoveConnection}
              />
            )
          }
          return (
            <Devices
              key="devices"
              x={x}
              onEject={onEject}
              onEjectDevice={onEjectDevice}
              onMount={onMount}
            />
          )
        })}
      </div>
      <SidebarMenu
        menu={menu}
        prefs={prefs}
        update={update}
        onTrash={onTrash}
        onEmptyTrash={onEmptyTrash}
        setMenu={setMenu}
        menuRef={menuRef}
        menuPosition={menuPosition}
      />
    </aside>
  )
}

import { t } from '@/i18n'
import { useMenuPosition } from '@/lib/menuPosition'
import { useRef, useState } from 'react'
import { PluginContextActions } from '@/components/plugins/PluginContextActions'
import { contextItems } from './items'
import { ContextMenuButton } from './ContextMenuButton'
import { SubmenuItem } from './SubmenuItem'
import type { ContextMenuProps } from './types'

export function FileContextMenu({
  menu,
  hint,
  open,
  previewEntry,
  setInfo,
  openBatchRename,
  platform,
  openTags,
  x,
  isRemotePath,
  run,
  copy,
  duplicate,
  terminal,
  notify,
  recycle,
  deletePermanently,
  restoreFromTrash,
  inTrash,
  setMenu,
}: ContextMenuProps) {
  const [submenuOpen, setSubmenuOpen] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)
  const menuPosition = useMenuPosition(menu?.x ?? 0, menu?.y ?? 0, menuRef, Boolean(menu))
  const allItems = menu
    ? contextItems({
        menu,
        hint,
        open,
        previewEntry,
        setInfo,
        openBatchRename,
        platform,
        openTags,
        x,
        isRemotePath,
        run,
        copy,
        duplicate,
        terminal,
        notify,
        recycle,
        deletePermanently,
        restoreFromTrash,
        inTrash,
      })
    : []
  const tailLabels = new Set([t('移到废纸篓'), t('彻底删除'), t('放回原处')])
  const items = allItems.filter((item) => !tailLabels.has(item.label))
  const tail = allItems.filter((item) => tailLabels.has(item.label))
  return (
    <>
      {menu && (
        <div
          ref={menuRef}
          role="menu"
          aria-label={t('文件操作')}
          className="context-menu"
          style={menuPosition}
        >
          {items.map((a) =>
            'submenu' in a ? (
              <SubmenuItem
                key={a.label}
                item={a}
                open={submenuOpen}
                setOpen={setSubmenuOpen}
                menuPosition={menuPosition}
                setMenu={setMenu}
              />
            ) : (
              <ContextMenuButton key={a.label} item={a} setMenu={setMenu} />
            ),
          )}
          <PluginContextActions
            paths={x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path]}
            path={x.path}
            isDirectory={menu.entry.isDir}
            readOnly={x.readOnly}
            isRemote={isRemotePath(menu.entry.path)}
            onClose={() => setMenu(null)}
            onError={x.setError}
          />
          {tail.map((a) =>
            'submenu' in a ? (
              <SubmenuItem
                key={a.label}
                item={a}
                open={submenuOpen}
                setOpen={setSubmenuOpen}
                menuPosition={menuPosition}
                setMenu={setMenu}
              />
            ) : (
              <ContextMenuButton key={a.label} item={a} setMenu={setMenu} />
            ),
          )}
        </div>
      )}
    </>
  )
}

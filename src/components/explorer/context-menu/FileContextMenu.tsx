import { t } from '@/i18n'
import { useMenuPosition } from '@/lib/menuPosition'
import { useEffect, useRef, useState } from 'react'
import {
  CloudCheck,
  CloudDownload,
  CloudOff,
  Copy,
  ExternalLink,
  FilePlus,
  FolderOpen,
  FolderPlus,
  Eye,
  Info,
  AppWindow,
  Pencil,
  PinOff,
  RotateCcw,
  Scissors,
  Tags,
  Trash2,
} from 'lucide-react'
import { PluginContextActions } from '@/components/plugins/PluginContextActions'
import {
  PluginMenuNode,
  SubmenuGroupContext,
  type PluginMenuItem,
} from '@/components/plugins/PluginMenuNode'
import { MenuLayer } from '@/components/ui/MenuLayer'
import { OpenWithPicker } from './OpenWithPicker'
import { useContextMenu } from './useContextMenu'
import { executeCoreMenu } from './execute'
import type { CoreMenu, CoreMenuItem } from './useContextMenu'
import type { ContextMenuProps } from './types'

const icons = {
  open: ExternalLink,
  reveal: FolderOpen,
  eye: Eye,
  info: Info,
  pencil: Pencil,
  tags: Tags,
  copy: Copy,
  scissors: Scissors,
  trash: Trash2,
  restore: RotateCcw,
  'cloud-check': CloudCheck,
  'cloud-download': CloudDownload,
  'cloud-off': CloudOff,
  'pin-off': PinOff,
  'file-plus': FilePlus,
  'folder-plus': FolderPlus,
  application: AppWindow,
  'application-search': AppWindow,
}

export function FileContextMenu(props: ContextMenuProps) {
  const { menu, x, setMenu } = props
  const ref = useRef<HTMLDivElement>(null)
  const [openWithPath, setOpenWithPath] = useState('')
  // Hooks run in a stable order, but the menu element only exists after the
  // Core snapshot is ready. Keep `open` false until then so the measuring
  // effect runs again when the real node mounts (otherwise the stale initial
  // 0,0 placement would survive).
  const {
    paths,
    menus,
    error,
    ready,
    pluginMenus,
    pluginLoading,
    pluginError,
    retryPlugins,
    resolveCoreChildren,
  } = useContextMenu(props)
  const [openCoreChild, setOpenCoreChild] = useState<string | null>(null)
  const setError = x.setError
  useEffect(() => {
    // Menu resolution errors use the one modal error channel, never an inline
    // row that would resize the popup.
    if (error) setError(error)
  }, [error, setError])
  const position = useMenuPosition(menu?.x ?? 0, menu?.y ?? 0, ref, Boolean(menu) && ready)
  if (!menu)
    return openWithPath ? (
      <OpenWithPicker
        path={openWithPath}
        onClose={() => setOpenWithPath('')}
        onError={x.setError}
      />
    ) : null
  // Wait for the Core-only snapshot: the menu must appear with its final
  // top-level height instead of growing as providers answer.
  if (!ready) return null
  if (error) return null
  const render = (group: CoreMenu, item: CoreMenuItem) => {
    if (item.kind === 'separator')
      return <div key={item.id} role="separator" className="context-divider" />
    if (item.kind === 'submenu') {
      return (
        <PluginMenuNode
          key={`${group.providerId}:${item.id}`}
          item={item as PluginMenuItem}
          items={group.items as PluginMenuItem[]}
          onAction={(selected) => {
            const coreItem = group.items.find((candidate) => candidate.id === selected.id)
            if (coreItem) {
              setMenu(null)
              void executeCoreMenu(group, coreItem, props, setOpenWithPath)
            }
          }}
          onExpand={(parentId) => {
            void resolveCoreChildren(group, parentId).catch((reason) => x.setError(String(reason)))
          }}
        />
      )
    }
    const Icon = icons[item.icon as keyof typeof icons]
    return (
      <button
        key={`${group.providerId}:${item.id}`}
        role="menuitem"
        disabled={item.disabled}
        className={item.danger ? 'context-danger' : undefined}
        onClick={() => {
          setMenu(null)
          void executeCoreMenu(group, item, props, setOpenWithPath)
        }}
      >
        {Icon && <Icon />}
        {item.title}
        {item.shortcut && <span className="shortcut">{props.hint(item.shortcut)}</span>}
      </button>
    )
  }
  return (
    <>
      <MenuLayer>
        <div
          ref={ref}
          role="menu"
          aria-label={t('文件操作')}
          className="context-menu"
          style={position}
        >
          <SubmenuGroupContext.Provider
            value={{ openId: openCoreChild, setOpenId: setOpenCoreChild }}
          >
            {menus.flatMap((group) =>
              group.items
                .filter((item) => !item.parentId && item.placement !== 'tail')
                .map((item) => render(group, item)),
            )}
          </SubmenuGroupContext.Provider>
          <PluginContextActions
            paths={paths}
            path={x.path}
            menus={pluginMenus}
            loading={pluginLoading}
            error={pluginError}
            onRetry={retryPlugins}
            onClose={() => setMenu(null)}
            onError={x.setError}
          />
          <SubmenuGroupContext.Provider
            value={{ openId: openCoreChild, setOpenId: setOpenCoreChild }}
          >
            {menus.flatMap((group) =>
              group.items
                .filter((item) => !item.parentId && item.placement === 'tail')
                .map((item) => render(group, item)),
            )}
          </SubmenuGroupContext.Provider>
        </div>
      </MenuLayer>
      {openWithPath && (
        <OpenWithPicker
          path={openWithPath}
          onClose={() => setOpenWithPath('')}
          onError={x.setError}
        />
      )}
    </>
  )
}

import { t } from '@/i18n'
import { useMenuPosition } from '@/lib/menuPosition'
import { useEffect, useRef } from 'react'
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
  Pencil,
  PinOff,
  RotateCcw,
  Scissors,
  Tags,
  TerminalSquare,
  Trash2,
} from 'lucide-react'
import { PluginContextActions } from '@/components/plugins/PluginContextActions'
import { usePluginCatalog } from '@/lib/plugins'
import { MenuLayer } from '@/components/ui/MenuLayer'
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
  terminal: TerminalSquare,
  trash: Trash2,
  restore: RotateCcw,
  'cloud-check': CloudCheck,
  'cloud-download': CloudDownload,
  'cloud-off': CloudOff,
  'pin-off': PinOff,
  'file-plus': FilePlus,
  'folder-plus': FolderPlus,
}

export function FileContextMenu(props: ContextMenuProps) {
  const { menu, x, setMenu } = props
  const ref = useRef<HTMLDivElement>(null)
  // Hooks run in a stable order, but the menu element only exists after the
  // Core snapshot is ready. Keep `open` false until then so the measuring
  // effect runs again when the real node mounts (otherwise the stale initial
  // 0,0 placement would survive).
  const { paths, menus, error, ready } = useContextMenu(props)
  const plugins = usePluginCatalog()
  const setError = x.setError
  useEffect(() => {
    // Menu resolution errors use the one modal error channel, never an inline
    // row that would resize the popup.
    if (error) setError(error)
  }, [error, setError])
  const position = useMenuPosition(menu?.x ?? 0, menu?.y ?? 0, ref, Boolean(menu) && ready)
  const virtual = props.x.path === 'applications://' || props.x.path === 'recyclebin://'
  const pluginEntry =
    !virtual &&
    paths.length > 0 &&
    plugins.some((plugin) => plugin.enabled && !!plugin.extensions?.contextMenus)
  const populated = menus.some((group) => group.items.length > 0) || pluginEntry
  if (!menu) return null
  // Wait for the Core-only snapshot: the menu must appear with its final
  // top-level height instead of growing as providers answer.
  if (!ready) return null
  if (error) return null
  // Never leave an empty white popup when every provider declined and the
  // plugin entry is not applicable (for example a blank canvas).
  if (!populated) return null
  const render = (group: CoreMenu, item: CoreMenuItem) => {
    const Icon = icons[item.icon as keyof typeof icons]
    return (
      <button
        key={`${group.providerId}:${item.id}`}
        role="menuitem"
        disabled={item.disabled}
        className={item.danger ? 'context-danger' : undefined}
        onClick={() => {
          setMenu(null)
          void executeCoreMenu(group, item, props)
        }}
      >
        {Icon && <Icon />}
        {item.title}
        {item.shortcut && <span className="shortcut">{props.hint(item.shortcut)}</span>}
      </button>
    )
  }
  return (
    <MenuLayer>
      <div
        ref={ref}
        role="menu"
        aria-label={t('文件操作')}
        className="context-menu"
        style={position}
      >
        {menus.flatMap((group) =>
          group.items
            .filter((item) => item.placement !== 'tail')
            .map((item) => render(group, item)),
        )}
        {!virtual && pluginEntry && (
          <PluginContextActions
            paths={paths}
            path={x.path}
            onClose={() => setMenu(null)}
            onError={x.setError}
          />
        )}
        {menus.flatMap((group) =>
          group.items
            .filter((item) => item.placement === 'tail')
            .map((item) => render(group, item)),
        )}
      </div>
    </MenuLayer>
  )
}

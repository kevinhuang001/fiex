import { t } from '@/i18n'
import { useMenuPosition } from '@/lib/menuPosition'
import { useMemo, useRef } from 'react'
import {
  CloudCheck,
  CloudDownload,
  CloudOff,
  Copy,
  ExternalLink,
  FilePlus,
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
import { MenuLayer } from '@/components/ui/MenuLayer'
import { useContextMenu } from './useContextMenu'
import { executeCoreMenu } from './execute'
import type { CoreMenu, CoreMenuItem } from './useContextMenu'
import type { ContextMenuProps } from './types'

const icons = {
  open: ExternalLink,
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
  const position = useMenuPosition(menu?.x ?? 0, menu?.y ?? 0, ref, Boolean(menu))
  const { paths, menus, error } = useContextMenu(props)
  const virtual = props.x.path === 'applications://' || props.x.path === 'recyclebin://'
  const builtin = useMemo(
    () => menus.filter((menu): menu is CoreMenu => menu.kind === 'builtin'),
    [menus],
  )
  const plugins = useMemo(() => menus.filter((menu) => menu.kind === 'plugin'), [menus])
  if (!menu) return null
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
        {builtin.flatMap((group) =>
          group.items
            .filter((item) => item.placement !== 'tail')
            .map((item) => render(group, item)),
        )}
        {!virtual && (
          <PluginContextActions
            paths={paths}
            path={x.path}
            resolvedMenus={plugins}
            onClose={() => setMenu(null)}
            onError={x.setError}
          />
        )}
        {builtin.flatMap((group) =>
          group.items
            .filter((item) => item.placement === 'tail')
            .map((item) => render(group, item)),
        )}
        {error && <p role="alert">{error}</p>}
      </div>
    </MenuLayer>
  )
}

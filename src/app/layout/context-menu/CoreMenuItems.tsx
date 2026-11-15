import {
  AppWindow,
  CloudCheck,
  CloudDownload,
  CloudOff,
  Copy,
  ExternalLink,
  Eye,
  FilePlus,
  FolderOpen,
  FolderPlus,
  Info,
  Pencil,
  PinOff,
  RotateCcw,
  Scissors,
  Tags,
  Trash2,
} from 'lucide-react'
import { PluginMenuNode, type PluginMenuItem } from '@/components/plugins/PluginMenuNode'
import { executeCoreMenu } from '@/features/context-menu/services/executeCoreMenu'
import { isMoreAction } from '@/features/context-menu/model/moreMenu'
import type { ContextMenuProps } from '@/features/context-menu/types'
import type { CoreMenu, CoreMenuItem } from '@/lib/coreMenu'
import { contextMenuExecutionServices } from './contextMenuAdapters'

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

export function CoreMenuItems({
  menus,
  actions,
  hint,
  setMenu,
  setOpenChild,
  onOpenWith,
  onError,
  resolveCoreChildren,
  placement,
}: {
  menus: CoreMenu[]
  actions: ContextMenuProps
  hint: ContextMenuProps['hint']
  setMenu: ContextMenuProps['setMenu']
  setOpenChild: (id: string | null) => void
  onOpenWith: (path: string) => void
  onError: (message: string) => void
  resolveCoreChildren: (group: CoreMenu, parentId: string) => Promise<void>
  placement: 'regular' | 'tail'
}) {
  const render = (group: CoreMenu, item: CoreMenuItem) => {
    if (item.kind === 'separator')
      return <div key={item.id} role="separator" className="context-divider" />
    if (item.kind === 'submenu')
      return (
        <PluginMenuNode
          key={`${group.providerId}:${item.id}`}
          item={item as PluginMenuItem}
          items={group.items as PluginMenuItem[]}
          onAction={(selected) => {
            const action = group.items.find((candidate) => candidate.id === selected.id)
            if (!action) return
            setMenu(null)
            void executeCoreMenu(group, action, actions, contextMenuExecutionServices, onOpenWith)
          }}
          onExpand={(parentId) => {
            void resolveCoreChildren(group, parentId).catch((error) => onError(String(error)))
          }}
        />
      )

    const Icon = icons[item.icon as keyof typeof icons]
    return (
      <button
        key={`${group.providerId}:${item.id}`}
        role="menuitem"
        disabled={item.disabled}
        className={item.danger ? 'context-danger' : undefined}
        onMouseEnter={() => setOpenChild(null)}
        onClick={() => {
          setMenu(null)
          void executeCoreMenu(group, item, actions, contextMenuExecutionServices, onOpenWith)
        }}
      >
        {Icon && <Icon />}
        {item.title}
        {item.shortcut && <span className="shortcut">{hint(item.shortcut)}</span>}
      </button>
    )
  }

  return (
    <>
      {menus.flatMap((group) =>
        group.items
          .filter(
            (item) =>
              !item.parentId &&
              (placement === 'tail' ? item.placement === 'tail' : item.placement !== 'tail') &&
              !isMoreAction(item.id),
          )
          .map((item) => render(group, item)),
      )}
    </>
  )
}

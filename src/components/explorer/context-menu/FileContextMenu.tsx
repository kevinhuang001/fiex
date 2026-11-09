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
import { hasTopLevelMenuActions } from './menuVisibility'
import { buildMoreMenuPlan, isMoreAction } from './moreMenu'
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
  const hasCoreActions = hasTopLevelMenuActions(menus)
  const hasPluginActions = hasTopLevelMenuActions(pluginMenus)
  const morePlan = buildMoreMenuPlan(
    menus,
    paths,
    props.workspace.items.map((entry) => entry.path),
  )
  const hasWorkspaceAction = Boolean(morePlan.workspaceAction)
  const moreMenuTrigger: PluginMenuItem = {
    id: 'more',
    kind: 'submenu',
    title: t('更多'),
    icon: 'more',
  }
  const moreMenuItems: PluginMenuItem[] = [
    moreMenuTrigger,
    ...morePlan.coreActions.map(({ item }) => ({
      id: item.id,
      kind: 'action' as const,
      parentId: 'more',
      title: item.title,
      icon: item.icon,
      disabled: item.disabled,
      danger: item.danger,
      shortcut: item.shortcut ? props.hint(item.shortcut) : undefined,
    })),
    ...(morePlan.workspaceAction
      ? [
          {
            id: 'workspace',
            kind: 'action' as const,
            parentId: 'more',
            title: morePlan.workspaceAction.kind === 'remove' ? t('移出工作区') : t('添加到工作区'),
            icon: morePlan.workspaceAction.kind === 'remove' ? 'list-minus' : 'list-plus',
          },
        ]
      : []),
  ]
  useEffect(() => {
    if (
      menu &&
      ready &&
      !error &&
      !pluginLoading &&
      !pluginError &&
      !hasCoreActions &&
      !hasPluginActions &&
      !hasWorkspaceAction
    )
      setMenu(null)
  }, [
    menu,
    ready,
    error,
    pluginLoading,
    pluginError,
    hasCoreActions,
    hasPluginActions,
    hasWorkspaceAction,
    setMenu,
  ])
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
  if (!hasCoreActions && pluginLoading && !hasWorkspaceAction) return null
  if (!hasCoreActions && !pluginLoading && !pluginError && !hasPluginActions && !hasWorkspaceAction)
    return null
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
                .filter(
                  (item) => !item.parentId && item.placement !== 'tail' && !isMoreAction(item.id),
                )
                .map((item) => render(group, item)),
            )}
          </SubmenuGroupContext.Provider>
          {moreMenuItems.length > 1 && (
            <PluginMenuNode
              item={moreMenuTrigger}
              items={moreMenuItems}
              onAction={(selected) => {
                const action = morePlan.coreActions.find(({ item }) => item.id === selected.id)
                if (action) {
                  setMenu(null)
                  void executeCoreMenu(action.group, action.item, props, setOpenWithPath)
                  return
                }
                const workspaceAction = morePlan.workspaceAction
                if (selected.id !== 'workspace' || !workspaceAction) return
                setMenu(null)
                if (workspaceAction.kind === 'remove') {
                  workspaceAction.paths.forEach((path) => props.workspace.remove(path))
                } else {
                  void props
                    .addWorkspace(workspaceAction.paths)
                    .then(() => props.workspace.setOpen(true))
                }
              }}
              onExpand={() => {}}
            />
          )}
          {(pluginLoading || pluginError || hasPluginActions) && (
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
          )}
          <SubmenuGroupContext.Provider
            value={{ openId: openCoreChild, setOpenId: setOpenCoreChild }}
          >
            {menus.flatMap((group) =>
              group.items
                .filter(
                  (item) => !item.parentId && item.placement === 'tail' && !isMoreAction(item.id),
                )
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

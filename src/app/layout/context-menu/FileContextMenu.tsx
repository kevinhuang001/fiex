import { t } from '@/i18n'
import { useMenuPosition } from '@/lib/menuPosition'
import { useEffect, useMemo, useRef, useState } from 'react'
import { PluginContextActions } from '@/components/plugins/PluginContextActions'
import {
  PluginMenuNode,
  SubmenuGroupContext,
  type PluginMenuItem,
} from '@/components/plugins/PluginMenuNode'
import { MenuLayer } from '@/components/ui/MenuLayer'
import { OpenWithPicker } from './OpenWithPicker'
import { useContextMenu } from '@/features/context-menu/hooks/useContextMenu'
import { hasTopLevelMenuActions } from '@/features/context-menu/model/menuVisibility'
import { buildMoreMenuPlan } from '@/features/context-menu/model/moreMenu'
import { executeCoreMenu } from '@/features/context-menu/services/executeCoreMenu'
import type { ContextMenuProps } from '@/features/context-menu/types'
import { CoreMenuItems } from './CoreMenuItems'
import { contextMenuExecutionServices, contextMenuSessionServices } from './contextMenuAdapters'

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
  } = useContextMenu(props, contextMenuSessionServices)
  const [openCoreChild, setOpenCoreChild] = useState<string | null>(null)
  const rootSubmenuGroup = useMemo(
    () => ({ openId: openCoreChild, setOpenId: setOpenCoreChild }),
    [openCoreChild],
  )
  const setError = x.setError
  const hasCoreActions = hasTopLevelMenuActions(menus)
  const hasPluginActions = hasTopLevelMenuActions(pluginMenus)
  const morePlan = buildMoreMenuPlan(
    menus,
    paths,
    props.workspace.items.map((entry) => entry.path),
    props.x.path,
  )
  const hasWorkspaceAction = Boolean(morePlan.workspaceAction)
  useEffect(() => {
    setOpenCoreChild(null)
  }, [menu])
  const moreMenuTrigger: PluginMenuItem = {
    id: 'more',
    kind: 'submenu',
    title: t('更多'),
    icon: 'more',
  }
  const moreActionItem = ({ item }: (typeof morePlan.coreActions)[number]): PluginMenuItem => ({
    id: item.id,
    kind: 'action',
    parentId: 'more',
    title: item.title,
    icon: item.icon,
    disabled: item.disabled,
    danger: item.danger,
    shortcut: item.shortcut ? props.hint(item.shortcut) : undefined,
  })
  const regularMoreActions = morePlan.coreActions.filter(({ item }) => item.id !== 'delete')
  const permanentDeleteActions = morePlan.coreActions.filter(({ item }) => item.id === 'delete')
  const moreMenuItems: PluginMenuItem[] = [
    moreMenuTrigger,
    ...regularMoreActions.map(moreActionItem),
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
    ...(permanentDeleteActions.length
      ? [
          {
            id: 'permanent-delete-separator',
            kind: 'separator' as const,
            parentId: 'more',
            title: '',
          },
        ]
      : []),
    ...permanentDeleteActions.map(moreActionItem),
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
          <SubmenuGroupContext.Provider value={rootSubmenuGroup}>
            <CoreMenuItems
              placement="regular"
              menus={menus}
              actions={props}
              hint={props.hint}
              setMenu={setMenu}
              setOpenChild={setOpenCoreChild}
              onOpenWith={setOpenWithPath}
              onError={x.setError}
              resolveCoreChildren={resolveCoreChildren}
            />
            {moreMenuItems.length > 1 && (
              <PluginMenuNode
                item={moreMenuTrigger}
                items={moreMenuItems}
                onAction={(selected) => {
                  const action = morePlan.coreActions.find(({ item }) => item.id === selected.id)
                  if (action) {
                    setMenu(null)
                    void executeCoreMenu(
                      action.group,
                      action.item,
                      props,
                      contextMenuExecutionServices,
                      setOpenWithPath,
                    )
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
            <CoreMenuItems
              placement="tail"
              menus={menus}
              actions={props}
              hint={props.hint}
              setMenu={setMenu}
              setOpenChild={setOpenCoreChild}
              onOpenWith={setOpenWithPath}
              onError={x.setError}
              resolveCoreChildren={resolveCoreChildren}
            />
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

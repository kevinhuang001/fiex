import { t } from '@/i18n'
import { call } from '@/lib/api'
import { requestDirectoryRefresh } from '@/lib/listing'
import { reportFilesystemPermissionError } from '@/lib/errorCenter'
import { progressChannel, quietProgressChannel } from '@/hooks/useActivity'
import type { ContextMenuProps } from './types'
import type { CoreMenu, CoreMenuItem } from './useContextMenu'

export interface CoreAction {
  kind: 'coreAction'
  command: string
  paths: string[]
  path: string
  directory: string
}

/** Fixed Core-owned UI command adapter. Availability and selection are decided
 * by the backend provider/session, not reconstructed from the current pane. */
export function applyCoreAction(
  effect: CoreAction,
  props: ContextMenuProps,
  chooseOpenWith?: (path: string) => void,
) {
  const { x } = props
  const entry =
    props.menu?.entry?.path === effect.path
      ? props.menu.entry
      : x.entries.find((entry) => entry.path === effect.path)
  switch (effect.command) {
    case 'open':
      if (entry) props.open(entry)
      break
    case 'reveal':
      return (async () => {
        const alias = effect.paths[0]
        if (!alias) throw new Error('Recent navigation requires one selected alias')
        const target = await call('recent_navigation_target', {
          alias,
        })
        x.navigateTo(target.parent, target.target)
      })()
    case 'preview':
      if (entry) props.previewEntry(entry)
      break
    case 'info':
      if (entry) props.setInfo(entry)
      break
    case 'tags':
      props.openTags(effect.paths)
      break
    case 'rename':
      x.beginRename(effect.path)
      break
    case 'batchRename':
      props.openBatchRename(effect.paths)
      break
    case 'copy':
      props.copy(false, effect.paths)
      break
    case 'cut':
      props.copy(true, effect.paths)
      break
    case 'duplicate':
      props.duplicate(effect.paths)
      break
    case 'trash':
      props.recycle(effect.paths)
      break
    case 'newFolder':
      props.create()
      break
    case 'newFile':
      props.createFile()
      break
    case 'restore':
      props.restoreFromTrash(effect.paths)
      break
    case 'delete':
      props.deletePermanently(effect.paths)
      break
    case 'emptyTrash':
      props.emptyTrash()
      break
    case 'removeFromRecent':
      requestDirectoryRefresh([effect.directory])
      break
    case 'copyPath':
      return navigator.clipboard.writeText(effect.path).then(() => props.notify(t('路径已复制')))
    case 'chooseOpenWith':
      chooseOpenWith?.(effect.path)
      break
    default:
      throw new Error('Unknown Core menu command')
  }
}
export async function executeCoreMenu(
  menu: CoreMenu,
  item: CoreMenuItem,
  props: ContextMenuProps,
  chooseOpenWith?: (path: string) => void,
) {
  let onProgress: ReturnType<typeof progressChannel> | undefined = quietProgressChannel()
  const execute = async () => {
    const effect = await call('plugin_context_menu_execute', {
      menuSessionId: menu.menuSessionId,
      itemId: item.id,
      onProgress,
    })
    if (
      effect !== null &&
      !Array.isArray(effect) &&
      typeof effect === 'object' &&
      effect.kind === 'coreAction' &&
      typeof effect.command === 'string' &&
      Array.isArray(effect.paths) &&
      typeof effect.path === 'string' &&
      typeof effect.directory === 'string'
    ) {
      await applyCoreAction(effect as unknown as CoreAction, props, chooseOpenWith)
      return true
    }
    requestDirectoryRefresh([props.x.path])
    return false
  }
  try {
    if (item.activity)
      await props.run(
        item.activity.label,
        execute,
        t('{0} 个项目', props.x.selected.length || 1),
        true,
        item.activity.kind,
        (id) => {
          onProgress = progressChannel(id) ?? quietProgressChannel()
        },
        item.activity.quiet,
        undefined,
        undefined,
        (coreAction) => (coreAction === undefined ? [props.x.path] : []),
      )
    else await execute()
  } catch (error) {
    const location = props.x.selected.length ? props.x.selected.join('\n') : props.x.path
    if (!reportFilesystemPermissionError(error, `${item.title}\n${location}`)) {
      props.x.setError(String(error))
    }
  }
}

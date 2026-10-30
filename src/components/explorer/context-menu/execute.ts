import { t } from '@/i18n'
import { call } from '@/lib/api'
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
export function applyCoreAction(effect: CoreAction, props: ContextMenuProps) {
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
      // Core resolves Recent aliases to local sources inside the command; the
      // webview only ever passes the opaque alias.
      return (async () => {
        for (const path of effect.paths) await call('reveal_path', { path })
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
    case 'trashApplications':
      return call('trash_application_entries', { paths: effect.paths }).then(() => {
        props.x.setSelected([])
        props.x.reload()
      })
    case 'removeApplications':
      return call('remove_application_entries', { paths: effect.paths }).then(() => {
        props.x.setSelected([])
        props.x.reload()
      })
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
    case 'terminal':
      props.terminal()
      break
    case 'copyPath':
      return navigator.clipboard.writeText(effect.path).then(() => props.notify(t('路径已复制')))
    default:
      throw new Error('Unknown Core menu command')
  }
}
export async function executeCoreMenu(menu: CoreMenu, item: CoreMenuItem, props: ContextMenuProps) {
  let onProgress: ReturnType<typeof progressChannel> | undefined = quietProgressChannel()
  const execute = async () => {
    const effect = await call<CoreAction | { handled: boolean }>('plugin_context_menu_execute', {
      menuSessionId: menu.menuSessionId,
      itemId: item.id,
      onProgress,
    })
    if ('kind' in effect && effect.kind === 'coreAction') await applyCoreAction(effect, props)
    else props.x.reload()
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
          onProgress = progressChannel(id)
        },
        item.activity.quiet,
      )
    else await execute()
  } catch (error) {
    props.x.setError(String(error))
  }
}

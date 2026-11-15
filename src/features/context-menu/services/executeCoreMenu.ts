import { t } from '@/i18n'
import { parseCoreMenuAction } from '@/lib/coreMenu'
import type { CoreMenu, CoreMenuAction, CoreMenuItem } from '@/lib/coreMenu'
import type { ContextMenuExecutionServices, ContextMenuProps } from '../types'

/** Fixed Core-owned UI command adapter. Availability and selection are decided
 * by the backend provider/session, not reconstructed from the current pane. */
export function applyCoreAction(
  effect: CoreMenuAction,
  props: ContextMenuProps,
  services: ContextMenuExecutionServices,
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
        const target = await services.recentNavigationTarget(alias)
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
    case 'newFile.empty':
      props.createFile()
      break
    case 'newFile.docx':
      props.createFile('docx')
      break
    case 'newFile.xlsx':
      props.createFile('xlsx')
      break
    case 'newFile.pptx':
      props.createFile('pptx')
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
      services.refreshDirectories([effect.directory])
      break
    case 'copyPath':
      return services.copyText(effect.path).then(() => props.notify(t('路径已复制')))
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
  services: ContextMenuExecutionServices,
  chooseOpenWith?: (path: string) => void,
) {
  let onProgress = services.quietProgressChannel()
  const execute = async () => {
    const effect = parseCoreMenuAction(
      await services.executeMenu(menu.menuSessionId, item.id, onProgress),
    )
    if (effect) {
      await applyCoreAction(effect, props, services, chooseOpenWith)
      return true
    }
    services.refreshDirectories([props.x.path])
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
          onProgress = services.progressChannel(id) ?? services.quietProgressChannel()
        },
        item.activity.quiet,
        undefined,
        undefined,
        (coreAction) => (coreAction === undefined ? [props.x.path] : []),
      )
    else await execute()
  } catch (error) {
    const location = props.x.selected.length ? props.x.selected.join('\n') : props.x.path
    if (!services.reportFilesystemPermissionError(error, `${item.title}\n${location}`)) {
      props.x.setError(String(error))
    }
  }
}

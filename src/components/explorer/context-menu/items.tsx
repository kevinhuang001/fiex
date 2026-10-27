import type { ContextItem, ContextItemsProps, ContextMenuProvider } from './types'
import { openingProvider } from './providers/opening'
import { tagsProvider } from './providers/tags'
import { connectionProvider } from './providers/connection'
import { renameProvider } from './providers/rename'
import { batchRenameProvider } from './providers/batchRename'
import { clipboardProvider } from './providers/clipboard'
import { terminalProvider } from './providers/terminal'
import { copyPathProvider } from './providers/copyPath'
import { trashProvider } from './providers/trash'

export const builtinMenuProviders: readonly ContextMenuProvider[] = [
  openingProvider,
  tagsProvider,
  connectionProvider,
  renameProvider,
  batchRenameProvider,
  clipboardProvider,
  terminalProvider,
  copyPathProvider,
  trashProvider,
]

/** Resolve one immutable, effective selection for every provider. Keeping UI
 * command providers local lets menus open synchronously without file IPC. */
export function resolveContextItems(
  props: ContextItemsProps,
  providers: readonly ContextMenuProvider[] = builtinMenuProviders,
): { items: ContextItem[]; tail: ContextItem[]; paths: string[] } {
  const paths = props.x.selected.includes(props.menu.entry.path)
    ? [...props.x.selected]
    : [props.menu.entry.path]
  const context = { ...props, paths, multiple: paths.length > 1 }
  const items: ContextItem[] = []
  const tail: ContextItem[] = []
  for (const provider of [...providers].sort(
    (a, b) => a.order - b.order || a.id.localeCompare(b.id),
  )) {
    if (!provider.matches(context)) continue
    const target = provider.placement === 'tail' ? tail : items
    target.push(
      ...provider.resolve(context).map((item, index) => ({
        ...item,
        id: `${provider.id}:${index}`,
      })),
    )
  }
  return { items, tail, paths }
}

export interface ContextMenuRequestKeyInput {
  x: number | undefined
  y: number | undefined
  directory: string
  paths: string[]
  locale: string
}

/** Use all selected items only when the clicked entry belongs to the selection. */
export function contextMenuPaths(entryPath: string | undefined, selected: string[]): string[] {
  if (!entryPath) return []
  return selected.includes(entryPath)
    ? [entryPath, ...selected.filter((path) => path !== entryPath)]
    : [entryPath]
}

/** Identifies one right-click snapshot, including presentation-affecting inputs. */
export function contextMenuRequestKey(input: ContextMenuRequestKeyInput): string {
  return JSON.stringify([input.x, input.y, input.directory, input.paths, input.locale])
}

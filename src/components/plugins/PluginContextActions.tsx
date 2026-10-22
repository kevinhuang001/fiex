import { useState } from 'react'
import { ChevronRight, Puzzle } from 'lucide-react'
import { call } from '@/lib/api'
import { usePluginCatalog } from '@/lib/plugins'
import { OPEN_PLUGIN_PANEL, PLUGIN_RESULT_EVENT } from '@/components/plugins/PluginPanelHost'

interface Item {
  id: string
  title: string
  when?: string
  command?: string
  panel?: { entry: string; title?: string; defaultSize?: { width: number; height: number } }
  resultView?: 'checksum' | 'json'
  children?: Item[]
  args?: Record<string, unknown>
  picker?: {
    mode: 'saveFile' | 'openFile' | 'openDirectory'
    suggestedName?: string
    extensions?: string[]
    param: string
  }
}
interface Context {
  paths: string[]
  path: string
  isDirectory: boolean
  readOnly: boolean
  isRemote: boolean
}
function matches(when: string | undefined, c: Context): boolean {
  if (!when) return true
  switch (when.trim()) {
    case 'context.selection.count > 0':
      return c.paths.length > 0
    case 'context.selection.count == 1':
      return c.paths.length === 1
    case 'context.selection.isFile':
      return c.paths.length === 1 && !c.isDirectory
    case 'context.selection.isDirectory':
      return c.paths.length === 1 && c.isDirectory
    case 'context.directory.canWrite':
      return !c.readOnly
    case 'context.directory.isLocal':
      return !c.isRemote
    default:
      return false
  }
}
function valid(nodes: Item[]): boolean {
  const ids = new Set<string>()
  let count = 0
  const check = (items: Item[], depth: number): boolean => {
    if (depth > 4) return false
    for (const item of items) {
      count++
      if (count > 100 || !item.id || !item.title || ids.has(item.id)) return false
      ids.add(item.id)
      if (
        Number(Boolean(item.children)) +
          Number(Boolean(item.command)) +
          Number(Boolean(item.panel)) !==
        1
      )
        return false
      if (item.children && (!item.children.length || !check(item.children, depth + 1))) return false
    }
    return true
  }
  return check(nodes, 1)
}
function MenuNode({
  item,
  onAction,
  ctx,
}: {
  item: Item
  onAction: (item: Item) => void
  ctx: Context
}) {
  const [expanded, setExpanded] = useState(false)
  if (!matches(item.when, ctx)) return null
  return item.children ? (
    <div
      className="context-submenu-wrap"
      onMouseEnter={() => setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
    >
      <button
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={expanded}
        onClick={(event) => {
          event.stopPropagation()
          setExpanded(!expanded)
        }}
      >
        <Puzzle size={15} />
        {item.title}
        <ChevronRight size={13} className="submenu-arrow" />
      </button>
      {expanded && (
        <div className="context-submenu" role="menu">
          {item.children.map((child) => (
            <MenuNode key={child.id} item={child} onAction={onAction} ctx={ctx} />
          ))}
        </div>
      )}
    </div>
  ) : (
    <button role="menuitem" onClick={() => onAction(item)}>
      <Puzzle size={15} />
      {item.title}
    </button>
  )
}
export function PluginContextActions({
  paths,
  path,
  isDirectory,
  readOnly,
  isRemote,
  onClose,
  onError,
  onChecksum,
}: Context & {
  onClose: () => void
  onError: (error: string) => void
  onChecksum?: (path: string) => void
}) {
  const plugins = usePluginCatalog()
  const ctx: Context = { paths, path, isDirectory, readOnly, isRemote }
  const invoke = async (pluginId: string, item: Item) => {
    onClose()
    if (item.panel) {
      window.dispatchEvent(
        new CustomEvent(OPEN_PLUGIN_PANEL, {
          detail: {
            pluginId,
            panelId: item.id,
            title: item.title,
            selection: paths,
            directory: path,
          },
        }),
      )
      return
    }
    if (!item.command) return
    if (item.resultView === 'checksum' && paths[0]) {
      onChecksum?.(paths[0])
      return
    }
    try {
      const args: Record<string, unknown> = { ...item.args, paths, path: paths[0], root: path }
      if (item.picker) {
        const selected = await call<string | null>('plugin_file_dialog', {
          mode: item.picker.mode,
          directory: path,
          suggestedName: item.picker.suggestedName,
          extensions: item.picker.extensions,
        })
        if (!selected) return
        args[item.picker.param] = selected
      }
      const response = await call<unknown>('plugin_invoke', {
        pluginId,
        method: item.command,
        params: args,
      })
      window.dispatchEvent(
        new CustomEvent(PLUGIN_RESULT_EVENT, { detail: { title: item.title, result: response } }),
      )
    } catch (error) {
      onError(String(error))
    }
  }
  return (
    <>
      {plugins
        .filter((p) => p.enabled)
        .flatMap((plugin) => {
          const items =
            (plugin.contributions as { contextMenus?: Item[] } | undefined)?.contextMenus || []
          if (!valid(items)) return []
          return items.map((item) => (
            <MenuNode
              key={plugin.id + ':' + item.id}
              item={item}
              ctx={ctx}
              onAction={(action) => void invoke(plugin.id, action)}
            />
          ))
        })}
    </>
  )
}

import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { ChevronRight, Puzzle } from 'lucide-react'
import { call } from '@/lib/api'
import { usePluginCatalog } from '@/lib/plugins'
import { beginActivity, finishActivity, updateActivity } from '@/hooks/useActivity'
import { OPEN_PLUGIN_PANEL, PLUGIN_RESULT_EVENT } from '@/components/plugins/PluginPanelHost'

interface Item {
  id: string
  title: string
  icon?: string
  when?: string
  command?: string
  panel?: { entry: string; title?: string; defaultSize?: { width: number; height: number } }
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
const archiveExtensions = new Set([
  'zip',
  'rar',
  '7z',
  'tar',
  'gz',
  'tgz',
  'bz2',
  'tbz2',
  'xz',
  'txz',
])
const isArchivePath = (path: string) =>
  archiveExtensions.has(path.split('.').pop()?.toLowerCase() || '')
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
    case 'context.selection.isArchive':
      return c.paths.length === 1 && !c.isDirectory && isArchivePath(c.paths[0])
    case 'context.selection.isNotArchive':
      return c.paths.length > 0 && c.paths.every((path) => !isArchivePath(path))
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
  onAction: (item: Item, anchor?: { x: number; y: number }) => void
  ctx: Context
}) {
  const [expanded, setExpanded] = useState(false)
  const [openUp, setOpenUp] = useState(false)
  const submenu = useRef<HTMLDivElement>(null)
  const closeTimer = useRef<number | null>(null)
  const cancelClose = () => {
    if (closeTimer.current !== null) {
      window.clearTimeout(closeTimer.current)
      closeTimer.current = null
    }
  }
  const openSubmenu = () => {
    cancelClose()
    setExpanded(true)
  }
  const scheduleClose = () => {
    cancelClose()
    closeTimer.current = window.setTimeout(() => {
      closeTimer.current = null
      setExpanded(false)
    }, 180)
  }
  useEffect(() => cancelClose, [])
  useLayoutEffect(() => {
    if (!expanded || !submenu.current) return
    const rect = submenu.current.getBoundingClientRect()
    setOpenUp(rect.bottom > window.innerHeight - 8)
  }, [expanded, item.children])
  if (!matches(item.when, ctx)) return null
  return item.children ? (
    <div className="context-submenu-wrap" onMouseEnter={openSubmenu} onMouseLeave={scheduleClose}>
      <button
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={expanded}
        onClick={(event) => {
          event.stopPropagation()
          cancelClose()
          setExpanded(!expanded)
        }}
      >
        <Puzzle size={15} />
        {item.title}
        <ChevronRight size={13} className="submenu-arrow" />
      </button>
      {expanded && (
        <div
          ref={submenu}
          className={`context-submenu${openUp ? ' open-up' : ''}`}
          role="menu"
          onMouseEnter={openSubmenu}
          onMouseLeave={scheduleClose}
        >
          {item.children.map((child) => (
            <MenuNode key={child.id} item={child} onAction={onAction} ctx={ctx} />
          ))}
        </div>
      )}
    </div>
  ) : (
    <button
      role="menuitem"
      onClick={(event) => onAction(item, { x: event.clientX, y: event.clientY })}
    >
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
}: Context & {
  onClose: () => void
  onError: (error: string) => void
}) {
  const plugins = usePluginCatalog()
  const ctx: Context = { paths, path, isDirectory, readOnly, isRemote }
  const invoke = async (pluginId: string, item: Item, anchor?: { x: number; y: number }) => {
    onClose()
    if (item.panel) {
      window.dispatchEvent(
        new CustomEvent(OPEN_PLUGIN_PANEL, {
          detail: {
            pluginId,
            panelId: item.id,
            title: item.title,
            icon: item.icon || plugins.find((plugin) => plugin.id === pluginId)?.icon,
            selection: paths,
            directory: path,
            anchor,
            size: item.panel.defaultSize,
          },
        }),
      )
      return
    }
    if (!item.command) return
    const plugin = plugins.find((entry) => entry.id === pluginId)
    let activityId = ''
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
      activityId = beginActivity(
        `${plugin?.name || '插件'} · ${item.title}`,
        `${paths.length} 个项目`,
        undefined,
        'plugin',
      )
      updateActivity(activityId, { indeterminate: true })
      const response = await call<unknown>('plugin_invoke', {
        pluginId,
        method: item.command,
        params: args,
      })
      finishActivity(
        activityId,
        undefined,
        typeof response === 'string' && response ? response : '已完成',
      )
      // A path or null result belongs in the activity detail, not a modal.
      if (response !== null && typeof response === 'object') {
        window.dispatchEvent(
          new CustomEvent(PLUGIN_RESULT_EVENT, { detail: { title: item.title, result: response } }),
        )
      }
    } catch (error) {
      if (activityId) finishActivity(activityId, String(error))
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
              onAction={(action, at) => void invoke(plugin.id, action, at)}
            />
          ))
        })}
    </>
  )
}

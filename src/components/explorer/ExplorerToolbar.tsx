import { Action, Choice, Label, Menu } from '@/components/Menus'
import { Button } from '@/components/ui/button'
import type { AppController } from '@/hooks/useAppController'
import type { Group, Sort, View } from '@/lib/types'
import { ArrowsDownUpIcon as ArrowsDownUp } from '@phosphor-icons/react/dist/csr/ArrowsDownUp'
import { CaretLeftIcon as ChevronLeft } from '@phosphor-icons/react/dist/csr/CaretLeft'
import { CaretRightIcon as ChevronRight } from '@phosphor-icons/react/dist/csr/CaretRight'
import { CodeIcon as Code2 } from '@phosphor-icons/react/dist/csr/Code'
import { ColumnsIcon as Columns3 } from '@phosphor-icons/react/dist/csr/Columns'
import { FolderPlusIcon as FolderPlus } from '@phosphor-icons/react/dist/csr/FolderPlus'
import { CardsThreeIcon as CardsThree } from '@phosphor-icons/react/dist/csr/CardsThree'
import { GitBranchIcon as GitBranch } from '@phosphor-icons/react/dist/csr/GitBranch'
import { SquaresFourIcon as Grid2X2 } from '@phosphor-icons/react/dist/csr/SquaresFour'
import { ImagesIcon as Image } from '@phosphor-icons/react/dist/csr/Images'
import { RowsIcon as List } from '@phosphor-icons/react/dist/csr/Rows'
import { SidebarSimpleIcon as PanelLeft } from '@phosphor-icons/react/dist/csr/SidebarSimple'
import { SlidersHorizontalIcon as Settings2 } from '@phosphor-icons/react/dist/csr/SlidersHorizontal'
import { TerminalWindowIcon as TerminalSquare } from '@phosphor-icons/react/dist/csr/TerminalWindow'
import { useEffect, useRef, useState, type PointerEvent } from 'react'
import { createPortal } from 'react-dom'
import { Pin, Trash2 } from 'lucide-react'
import { ArchiveIcon as Archive } from '@phosphor-icons/react/dist/csr/Archive'
import { ArrowsClockwiseIcon as RefreshCw } from '@phosphor-icons/react/dist/csr/ArrowsClockwise'
import { CloudIcon as Cloud } from '@phosphor-icons/react/dist/csr/Cloud'
import { ColumnsIcon as Columns2 } from '@phosphor-icons/react/dist/csr/Columns'
import { HardDriveIcon as HardDrive } from '@phosphor-icons/react/dist/csr/HardDrive'
import { HashIcon as Hash } from '@phosphor-icons/react/dist/csr/Hash'
import { PuzzlePieceIcon as Puzzle } from '@phosphor-icons/react/dist/csr/PuzzlePiece'
import { ScrollIcon as ScrollText } from '@phosphor-icons/react/dist/csr/Scroll'
import { useActivity } from '@/hooks/useActivity'
import { usePluginCatalog, type PluginContribution } from '@/lib/plugins'
import { call } from '@/lib/api'
import { OPEN_PLUGIN_PANEL } from '@/components/plugins/PluginPanelHost'
import type { ReactNode } from 'react'
const views: { id: View; name: string; icon: ReactNode }[] = [
  { id: 'icons', name: '图标', icon: <Grid2X2 weight="duotone" size={20} /> },
  { id: 'list', name: '列表', icon: <List weight="duotone" size={20} /> },
  { id: 'columns', name: '分栏', icon: <Columns3 weight="duotone" size={20} /> },
  { id: 'gallery', name: '画廊', icon: <Image weight="duotone" size={20} /> },
]
const groupOptions: [Group, string][] = [
  ['none', '无'],
  ['kind', '种类'],
  ['modified', '修改时间'],
  ['extension', '文件扩展名'],
  ['size', '文件大小'],
]
const sortOptions: [Sort, string][] = [
  ['name', '名称'],
  ['modified', '修改时间'],
  ['size', '大小'],
  ['kind', '种类'],
]
type PinBox = { left: number; top: number; width: number; height: number; center: number }
type PinDrag = {
  from: number
  to: number
  left: number
  top: number
  width: number
  height: number
  gap: number
  settling: boolean
}
function reorder<T>(items: T[], from: number, to: number) {
  const next = [...items]
  const [moved] = next.splice(from, 1)
  next.splice(to, 0, moved)
  return next
}

const pluginIconMap: Record<string, ReactNode> = {
  'git-branch': <GitBranch weight="duotone" size={20} />,
  archive: <Archive weight="duotone" size={20} />,
  hash: <Hash weight="duotone" size={20} />,
  cloud: <Cloud weight="duotone" size={20} />,
  'hard-drive': <HardDrive weight="duotone" size={20} />,
}
function pluginIcon(name?: string) {
  return pluginIconMap[name || ''] || <Puzzle weight="duotone" size={20} />
}
const pluginKey = (pluginId: string, toolId: string) => pluginId + '::' + toolId

export function ExplorerToolbar({
  sidebar,
  setSidebar,
  x,
  displayName,
  prefs,
  update,
  hint,
  create,
  ides,
  terminals,
  openIde,
  terminal,
  setSettings,
  refreshAll,
  inTrash,
  emptyTrash,
  split,
  onToggleSplit,
}: Pick<
  AppController,
  | 'sidebar'
  | 'setSidebar'
  | 'x'
  | 'displayName'
  | 'prefs'
  | 'update'
  | 'hint'
  | 'create'
  | 'ides'
  | 'terminals'
  | 'openIde'
  | 'terminal'
  | 'setSettings'
  | 'refreshAll'
  | 'inTrash'
  | 'emptyTrash'
  | 'displayName'
> & { split: boolean; onToggleSplit: () => void }) {
  const activity = useActivity()
  const plugins = usePluginCatalog()
  const lastPointer = useRef({ x: window.innerWidth - 18, y: 96 })
  const pinnedKeys = prefs.pinnedPluginTools || []
  const availableTools = plugins.flatMap((plugin) =>
    plugin.enabled ? (plugin.contributions?.toolbar || []).map((tool) => ({ plugin, tool })) : [],
  )
  const pinnedTools = pinnedKeys.flatMap((key) => {
    const found = availableTools.find(({ plugin, tool }) => pluginKey(plugin.id, tool.id) === key)
    return found ? [found] : []
  })
  const pinTool = (key: string) => {
    if (pinnedKeys.includes(key)) return
    update({ pinnedPluginTools: [...pinnedKeys, key] })
  }
  const unpinTool = (key: string) => {
    update({ pinnedPluginTools: pinnedKeys.filter((item) => item !== key) })
  }
  const autoPinned = useRef(false)
  useEffect(() => {
    if (!import.meta.env.DEV || autoPinned.current) return
    if (new URLSearchParams(location.search).has('pluginFixture')) return
    const defaults = availableTools
      .filter(({ plugin, tool }) => plugin.id.startsWith('org.fiex.') && tool.panel)
      .map(({ plugin, tool }) => pluginKey(plugin.id, tool.id))
    if (!defaults.length) return
    autoPinned.current = true
    const missing = defaults.filter((key) => !pinnedKeys.includes(key))
    if (missing.length) update({ pinnedPluginTools: [...pinnedKeys, ...missing] })
  }, [availableTools, pinnedKeys, update])
  useEffect(() => {
    const track = (event: globalThis.PointerEvent) => {
      lastPointer.current = { x: event.clientX, y: event.clientY }
    }
    window.addEventListener('pointermove', track, true)
    window.addEventListener('pointerdown', track, true)
    return () => {
      window.removeEventListener('pointermove', track, true)
      window.removeEventListener('pointerdown', track, true)
    }
  }, [])
  const runTool = (
    plugin: (typeof plugins)[number],
    tool: PluginContribution,
    anchor?: { x: number; y: number },
  ) => {
    if (tool.panel) {
      window.dispatchEvent(
        new CustomEvent(OPEN_PLUGIN_PANEL, {
          detail: {
            pluginId: plugin.id,
            panelId: tool.id,
            title: tool.title,
            icon: tool.icon,
            selection: x.selected,
            directory: x.path,
            anchor: anchor ?? lastPointer.current,
            source: 'toolbar',
          },
        }),
      )
      return
    }
    if (tool.command && plugin.backendAvailable) {
      void call('plugin_invoke', {
        pluginId: plugin.id,
        method: tool.command,
        params: { root: x.path },
      }).catch((error) => x.setError(String(error)))
      return
    }
    x.setError('插件尚未提供可用的工具面板')
  }
  const [pinDrag, setPinDrag] = useState<PinDrag | null>(null)
  const pinDragged = useRef(false)
  const pinOrigin = useRef<{
    pointerX: number
    grab: number
    rects: PinBox[]
    gap: number
  } | null>(null)
  const pinDetach = useRef<(() => void) | null>(null)
  const pinDropTimer = useRef<number | null>(null)
  const pinStrip = useRef<HTMLDivElement>(null)
  useEffect(
    () => () => {
      pinDetach.current?.()
      if (pinDropTimer.current !== null) window.clearTimeout(pinDropTimer.current)
    },
    [],
  )
  const beginPinDrag = (event: PointerEvent, from: number) => {
    if (event.button !== 0 || pinDrag || !pinStrip.current || !prefs.pinnedPluginTools) return
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
    const rects = Array.from(
      pinStrip.current.querySelectorAll<HTMLElement>('.plugin-tool-button'),
    ).map((element) => {
      const box = element.getBoundingClientRect()
      return {
        left: box.left,
        top: box.top,
        width: box.width,
        height: box.height,
        center: box.left + box.width / 2,
      }
    })
    const gap = rects.length > 1 ? Math.max(0, rects[1].left - (rects[0].left + rects[0].width)) : 4
    pinOrigin.current = { pointerX: event.clientX, grab: event.clientX - rect.left, rects, gap }
    pinDragged.current = false
    let started = false
    let to = from
    const move = (next: globalThis.PointerEvent) => {
      const data = pinOrigin.current
      if (!data) return
      if (!started) {
        if (Math.abs(next.clientX - data.pointerX) < 5) return
        started = true
        pinDragged.current = true
      }
      next.preventDefault()
      const box = data.rects[from]
      // Same rule as the tab strip: the pointer decides the slot, so a tool is
      // displaced once half of it is covered instead of a whole button later.
      let nextTo = 0
      for (let index = 0; index < data.rects.length; index++) {
        if (index !== from && next.clientX > data.rects[index].center) nextTo++
      }
      to = nextTo
      setPinDrag({
        from,
        to,
        left: next.clientX - data.grab,
        top: box.top,
        width: box.width,
        height: box.height,
        gap: data.gap,
        settling: false,
      })
    }
    const finish = (next: globalThis.PointerEvent) => {
      pinDetach.current?.()
      const data = pinOrigin.current
      pinOrigin.current = null
      if (!started || next.type === 'pointercancel' || !data) {
        setPinDrag(null)
        setTimeout(() => {
          pinDragged.current = false
        }, 0)
        return
      }
      const target = to
      setPinDrag((current) =>
        current ? { ...current, left: data.rects[target].left, settling: true } : current,
      )
      pinDropTimer.current = window.setTimeout(() => {
        pinDropTimer.current = null
        setPinDrag(null)
        if (target !== from) {
          update({ pinnedPluginTools: reorder(pinnedKeys, from, target) })
        }
        setTimeout(() => {
          pinDragged.current = false
        }, 0)
      }, 160)
    }
    const cleanup = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', finish)
      window.removeEventListener('pointercancel', finish)
      pinDetach.current = null
    }
    pinDetach.current = cleanup
    window.addEventListener('pointermove', move, { passive: false })
    window.addEventListener('pointerup', finish)
    window.addEventListener('pointercancel', finish)
  }
  const pinShift = (index: number) => {
    if (!pinDrag || index === pinDrag.from) return undefined
    const distance = pinDrag.width + pinDrag.gap
    if (pinDrag.from < pinDrag.to && index > pinDrag.from && index <= pinDrag.to)
      return `translate3d(-${distance}px, 0, 0)`
    if (pinDrag.to < pinDrag.from && index >= pinDrag.to && index < pinDrag.from)
      return `translate3d(${distance}px, 0, 0)`
    return undefined
  }
  const liftedPin = pinDrag && pinnedTools[pinDrag.from] ? pinDrag : null
  const [refreshKey, setRefreshKey] = useState(0)
  const [picker, setPicker] = useState<{
    kind: 'terminal' | 'ide'
    x: number
    y: number
  } | null>(null)
  useEffect(() => {
    if (!picker) return
    const close = (event: Event) => {
      if ((event.target as Element | null)?.closest('.toolbar-picker')) return
      setPicker(null)
    }
    window.addEventListener('pointerdown', close)
    window.addEventListener('blur', close)
    return () => {
      window.removeEventListener('pointerdown', close)
      window.removeEventListener('blur', close)
    }
  }, [picker])
  return (
    <>
      <header className="toolbar" data-tauri-drag-region>
        <Button
          variant="ghost"
          size="icon"
          aria-label="切换侧栏"
          title="切换侧栏"
          onClick={() => setSidebar(!sidebar)}
        >
          <PanelLeft weight="duotone" size={20} />
        </Button>
        <div className="navigation-controls">
          <Button
            variant="ghost"
            size="icon"
            disabled={!x.canBack}
            title="后退"
            aria-label="后退"
            onClick={x.back}
          >
            <ChevronLeft weight="duotone" size={20} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            disabled={!x.canForward}
            title="前进"
            aria-label="前进"
            onClick={x.forward}
          >
            <ChevronRight weight="duotone" size={20} />
          </Button>
        </div>
        <h1 data-tauri-drag-region>{x.query ? '搜索' : displayName(x.path) || 'Fiex'}</h1>
        <div className="toolbar-spacer" data-tauri-drag-region />
        <div className="view-switch">
          {views.map((v) => (
            <Button
              key={v.id}
              variant="ghost"
              size="icon"
              title={v.name}
              aria-label={`${v.name}视图`}
              aria-pressed={prefs.view === v.id}
              onClick={() => update({ view: v.id })}
            >
              {v.icon}
            </Button>
          ))}
        </div>
        <Menu label="分组方式" icon={<CardsThree weight="regular" size={20} />}>
          <Label>分组方式</Label>
          {groupOptions.map(([id, name]) => (
            <Choice key={id} checked={prefs.group === id} onClick={() => update({ group: id })}>
              {name}
            </Choice>
          ))}
        </Menu>
        <Menu label="排序方式" icon={<ArrowsDownUp weight="regular" size={20} />}>
          <Label>排序方式</Label>
          {sortOptions.map(([id, name]) => (
            <Choice key={id} checked={prefs.sort === id} onClick={() => update({ sort: id })}>
              {name}
            </Choice>
          ))}
          <Choice
            checked={prefs.descending}
            onClick={() => update({ descending: !prefs.descending })}
          >
            降序
          </Choice>
        </Menu>
        {inTrash && (
          <Button
            variant="ghost"
            size="icon"
            title="清空废纸篓"
            aria-label="清空废纸篓"
            onClick={emptyTrash}
          >
            <Trash2 size={20} />
          </Button>
        )}
        <Button
          variant="ghost"
          size="icon"
          title={`新建文件夹 ${hint('newFolder')}`}
          aria-label="新建文件夹"
          disabled={x.directoryReadOnly}
          onClick={() => create(true)}
        >
          <FolderPlus weight="duotone" size={20} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title={`在 ${ides.find((ide) => ide.id === prefs.ide)?.name || 'IDE'} 中打开`}
          aria-label="在 IDE 中打开"
          disabled={x.directoryReadOnly}
          onClick={() => openIde()}
          onContextMenu={(event) => {
            event.preventDefault()
            setPicker({ kind: 'ide', x: event.clientX, y: event.clientY })
          }}
        >
          <Code2 weight="regular" size={20} />
        </Button>
        <div className="terminal-control">
          <Button
            variant="ghost"
            size="icon"
            title={`在 ${terminals.find((t) => t.id === prefs.terminal)?.name || '终端'} 中打开`}
            aria-label="Open in Terminal"
            disabled={x.directoryReadOnly}
            onClick={() => terminal()}
            onContextMenu={(event) => {
              event.preventDefault()
              setPicker({ kind: 'terminal', x: event.clientX, y: event.clientY })
            }}
          >
            <TerminalSquare weight="regular" size={20} />
          </Button>
        </div>
        {availableTools.length > 0 && (
          <>
            <div className="toolbar-divider" />
            <div className="plugin-toolbar-group" aria-label="插件工具">
              <div className="plugin-pinned-strip" ref={pinStrip}>
                {pinnedTools.map(({ plugin, tool }, index) => {
                  const key = pluginKey(plugin.id, tool.id)
                  return (
                    <Button
                      key={key}
                      variant="ghost"
                      size="icon"
                      className={`plugin-tool-button ${pinDrag?.from === index ? 'dragging' : ''}`}
                      style={{ transform: pinShift(index) }}
                      title={`${plugin.name} · ${tool.title}（拖动排序，右键取消固定）`}
                      aria-label={`${plugin.name} ${tool.title}`}
                      onPointerDown={(event) => beginPinDrag(event, index)}
                      onContextMenu={(event) => {
                        event.preventDefault()
                        unpinTool(key)
                      }}
                      onClick={(event) => {
                        if (!pinDragged.current)
                          runTool(plugin, tool, { x: event.clientX, y: event.clientY })
                      }}
                    >
                      {pluginIcon(tool.icon)}
                    </Button>
                  )
                })}
              </div>
              <Menu label="插件" icon={<Puzzle weight="duotone" size={20} />}>
                <Label>插件工具</Label>
                {availableTools.map(({ plugin, tool }) => {
                  const key = pluginKey(plugin.id, tool.id)
                  const pinned = pinnedKeys.includes(key)
                  return (
                    <Action
                      key={key}
                      onClick={() => runTool(plugin, tool)}
                      trailing={
                        <button
                          type="button"
                          className={`plugin-pin ${pinned ? 'active' : ''}`}
                          aria-label={`${pinned ? '取消固定' : '固定'} ${plugin.name} ${tool.title}`}
                          title={pinned ? '取消固定' : '固定到工具栏'}
                          onClick={() => (pinned ? unpinTool(key) : pinTool(key))}
                        >
                          <Pin size={13} fill={pinned ? 'currentColor' : 'none'} />
                        </button>
                      }
                    >
                      <span className="plugin-menu-entry">
                        <span className="plugin-menu-icon">{pluginIcon(tool.icon)}</span>
                        <span className="plugin-menu-label">
                          {plugin.name} · {tool.title}
                        </span>
                      </span>
                    </Action>
                  )
                })}
                {!availableTools.length && <Label>暂无可用的插件工具</Label>}
              </Menu>
            </div>
            <div className="toolbar-divider" />
          </>
        )}
        <Button
          variant="ghost"
          size="icon"
          className="toolbar-secondary focus-visible:ring-0"
          title="刷新"
          aria-label="刷新"
          onClick={() => {
            setRefreshKey((key) => key + 1)
            refreshAll()
          }}
        >
          <RefreshCw
            key={refreshKey}
            weight="duotone"
            size={20}
            className={refreshKey ? 'refresh-turn' : ''}
          />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="toolbar-secondary focus-visible:ring-0"
          title={split ? '关闭当前面板' : '双面板'}
          aria-label={split ? '关闭当前面板' : '双面板'}
          aria-pressed={split}
          onClick={onToggleSplit}
        >
          <Columns2 weight="duotone" size={20} />
        </Button>

        <Button
          variant="ghost"
          size="icon"
          className="toolbar-secondary focus-visible:ring-0"
          title="状态中心"
          aria-label="状态中心"
          onClick={() => activity.setOpen(!activity.open)}
        >
          <ScrollText weight="duotone" size={20} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="toolbar-secondary focus-visible:ring-0"
          title="设置"
          aria-label="设置"
          onClick={() => setSettings(true)}
        >
          <Settings2 weight="duotone" size={20} />
        </Button>
      </header>
      {picker && (
        <div
          className="context-menu toolbar-picker"
          role="menu"
          aria-label={picker.kind === 'ide' ? '选择 IDE' : '选择终端'}
          style={{ left: picker.x, top: picker.y }}
        >
          {(picker.kind === 'ide' ? ides : terminals).map((item) => (
            <button
              role="menuitem"
              key={item.id}
              data-active={
                (picker.kind === 'ide' ? prefs.ide : prefs.terminal) === item.id || undefined
              }
              onClick={() => {
                update(picker.kind === 'ide' ? { ide: item.id } : { terminal: item.id })
                setPicker(null)
              }}
            >
              {item.name}
            </button>
          ))}
          {(picker.kind === 'ide' ? ides : terminals).length === 0 && (
            <button role="menuitem" disabled>
              未检测到可用应用
            </button>
          )}
        </div>
      )}
      {liftedPin &&
        createPortal(
          <div
            className={`plugin-tool-button plugin-tool-ghost ${liftedPin.settling ? 'settling' : ''}`}
            style={{
              width: liftedPin.width,
              height: liftedPin.height,
              transform: `translate3d(${liftedPin.left}px, ${liftedPin.top}px, 0) scale(1.08)`,
            }}
          >
            {pluginIcon(pinnedTools[liftedPin.from]?.tool.icon)}
          </div>,
          document.body,
        )}
    </>
  )
}

import { Button } from '@/components/ui/button'
import type { AppController } from '@/hooks/useAppController'
import { ArrowsClockwiseIcon as RefreshCw } from '@phosphor-icons/react/dist/csr/ArrowsClockwise'
import { CaretLeftIcon as ChevronLeft } from '@phosphor-icons/react/dist/csr/CaretLeft'
import { CaretRightIcon as ChevronRight } from '@phosphor-icons/react/dist/csr/CaretRight'
import { CodeIcon as Code2 } from '@phosphor-icons/react/dist/csr/Code'
import { ColumnsIcon as Columns2 } from '@phosphor-icons/react/dist/csr/Columns'
import { FolderPlusIcon as FolderPlus } from '@phosphor-icons/react/dist/csr/FolderPlus'
import { ScrollIcon as ScrollText } from '@phosphor-icons/react/dist/csr/Scroll'
import { SidebarSimpleIcon as PanelLeft } from '@phosphor-icons/react/dist/csr/SidebarSimple'
import { SlidersHorizontalIcon as Settings2 } from '@phosphor-icons/react/dist/csr/SlidersHorizontal'
import { TerminalWindowIcon as TerminalSquare } from '@phosphor-icons/react/dist/csr/TerminalWindow'
import { useState, type MouseEvent } from 'react'
import { Trash2 } from 'lucide-react'
import { useActivity } from '@/hooks/useActivity'
import { PinnedTools } from './toolbar/PinnedTools'
import { ToolbarPickerMenu, useToolbarPicker } from './toolbar/ToolbarMenus'
import { ViewControls } from './toolbar/ViewControls'
import { useToolbarOrder } from './toolbar/useToolbarOrder'

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
  | 'setSettings'
> & { split: boolean; onToggleSplit: () => void }) {
  const activity = useActivity()
  const [refreshKey, setRefreshKey] = useState(0)
  const { picker, pickerRef, pickerPosition, openPicker, closePicker } = useToolbarPicker()
  const leftDefaults = ['views', 'newFolder', 'ide', 'terminal']
  const rightDefaults = ['refresh', 'split', 'activity']
  const ordered = (saved: string[], fallback: string[]) => [
    ...saved.filter((id) => fallback.includes(id)),
    ...fallback.filter((id) => !saved.includes(id)),
  ]
  const leftKeys = ordered(prefs.toolbarLeftOrder, leftDefaults)
  const rightKeys = ordered(prefs.toolbarRightOrder, rightDefaults)
  const leftOrder = useToolbarOrder({
    keys: leftKeys,
    onChange: (toolbarLeftOrder) => update({ toolbarLeftOrder }),
  })
  const rightOrder = useToolbarOrder({
    keys: rightKeys,
    onChange: (toolbarRightOrder) => update({ toolbarRightOrder }),
  })
  const capture = (order: ReturnType<typeof useToolbarOrder>) => (event: MouseEvent) => {
    if (!order.suppressClick()) return
    event.preventDefault()
    event.stopPropagation()
  }
  const itemStyle = (order: ReturnType<typeof useToolbarOrder>, index: number) => {
    if (order.drag?.from === index)
      return {
        transform: `translateX(${order.drag.x}px)`,
        zIndex: 2,
        position: 'relative' as const,
      }
    const transform = order.shift(index)
    return transform ? { transform } : undefined
  }
  const renderLeft = (id: string) => {
    if (id === 'views') return <ViewControls prefs={prefs} update={update} />
    if (id === 'newFolder')
      return (
        <Button
          variant="ghost"
          size="icon"
          title={`新建文件夹 ${hint('newFolder')}`}
          aria-label="新建文件夹"
          disabled={x.directoryReadOnly}
          onClick={() => create(true)}
        >
          <FolderPlus weight="regular" size={20} />
        </Button>
      )
    if (id === 'ide')
      return (
        <Button
          variant="ghost"
          size="icon"
          title={`在 ${ides.find((ide) => ide.id === prefs.ide)?.name || 'IDE'} 中打开`}
          aria-label="在 IDE 中打开"
          disabled={x.directoryReadOnly}
          onClick={() => openIde()}
          onContextMenu={(event) => {
            event.preventDefault()
            openPicker('ide', event.clientX, event.clientY)
          }}
        >
          <Code2 weight="regular" size={20} />
        </Button>
      )
    return (
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
            openPicker('terminal', event.clientX, event.clientY)
          }}
        >
          <TerminalSquare weight="regular" size={20} />
        </Button>
      </div>
    )
  }
  const renderRight = (id: string) => {
    if (id === 'refresh')
      return (
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
            weight="regular"
            size={20}
            className={refreshKey ? 'refresh-turn' : ''}
          />
        </Button>
      )
    if (id === 'split')
      return (
        <Button
          variant="ghost"
          size="icon"
          className="toolbar-secondary focus-visible:ring-0"
          title={split ? '关闭当前面板' : '双面板'}
          aria-label={split ? '关闭当前面板' : '双面板'}
          aria-pressed={split}
          onClick={onToggleSplit}
        >
          <Columns2 weight="regular" size={20} />
        </Button>
      )
    return (
      <Button
        variant="ghost"
        size="icon"
        className="toolbar-secondary focus-visible:ring-0"
        title="状态中心"
        aria-label="状态中心"
        onClick={() => activity.setOpen(!activity.open)}
      >
        <ScrollText weight="regular" size={20} />
      </Button>
    )
  }
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
          <PanelLeft weight="regular" size={20} />
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
            <ChevronLeft weight="regular" size={20} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            disabled={!x.canForward}
            title="前进"
            aria-label="前进"
            onClick={x.forward}
          >
            <ChevronRight weight="regular" size={20} />
          </Button>
        </div>
        <h1 data-tauri-drag-region>{x.query ? '搜索' : displayName(x.path) || 'Fiex'}</h1>
        <div className="toolbar-spacer" data-tauri-drag-region />
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
        <div
          className="toolbar-order"
          ref={leftOrder.container}
          onClickCapture={capture(leftOrder)}
        >
          {leftKeys.map((id, index) => (
            <div
              className="toolbar-order-item"
              data-toolbar-item
              key={id}
              style={itemStyle(leftOrder, index)}
              onPointerDown={(event) => leftOrder.begin(event, index)}
            >
              {renderLeft(id)}
            </div>
          ))}
        </div>
        <PinnedTools prefs={prefs} update={update} x={x} />
        <div
          className="toolbar-order"
          ref={rightOrder.container}
          onClickCapture={capture(rightOrder)}
        >
          {rightKeys.map((id, index) => (
            <div
              className="toolbar-order-item"
              data-toolbar-item
              key={id}
              style={itemStyle(rightOrder, index)}
              onPointerDown={(event) => rightOrder.begin(event, index)}
            >
              {renderRight(id)}
            </div>
          ))}
        </div>
        <Button
          variant="ghost"
          size="icon"
          className="toolbar-secondary focus-visible:ring-0"
          title="设置"
          aria-label="设置"
          onClick={() => setSettings(true)}
        >
          <Settings2 weight="regular" size={20} />
        </Button>
      </header>
      {picker && (
        <ToolbarPickerMenu
          picker={picker}
          pickerRef={pickerRef}
          pickerPosition={pickerPosition}
          closePicker={closePicker}
          prefs={prefs}
          update={update}
          ides={ides}
          terminals={terminals}
        />
      )}
    </>
  )
}

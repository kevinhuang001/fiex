import { Choice, Label, Menu } from '@/components/Menus'
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
import { useEffect, useState } from 'react'
import { Columns2, RefreshCw, ScrollText, Trash2 } from 'lucide-react'
import { useActivity } from '@/hooks/useActivity'
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
export function ExplorerToolbar({
  sidebar,
  setSidebar,
  x,
  displayName,
  prefs,
  update,
  gitSwitched,
  hint,
  create,
  gitOpen,
  setGitOpen,
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
  | 'gitSwitched'
  | 'hint'
  | 'create'
  | 'gitOpen'
  | 'setGitOpen'
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
  const [refreshKey, setRefreshKey] = useState(0)
  const [picker, setPicker] = useState<{
    kind: 'terminal' | 'ide'
    x: number
    y: number
  } | null>(null)
  useEffect(() => {
    if (!picker) return
    const close = () => setPicker(null)
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
          title="刷新"
          aria-label="刷新"
          onClick={() => {
            setRefreshKey((key) => key + 1)
            refreshAll()
          }}
        >
          <RefreshCw key={refreshKey} size={20} className={refreshKey ? 'refresh-turn' : ''} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title={gitSwitched ? '历史版本只读' : `新建文件夹 ${hint('newFolder')}`}
          aria-label="新建文件夹"
          disabled={gitSwitched || x.directoryReadOnly}
          onClick={() => create(true)}
        >
          <FolderPlus weight="duotone" size={20} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className={`git-toolbar-button ${gitOpen ? 'active' : ''} ${gitSwitched ? 'switched' : ''}`}
          title={gitSwitched ? `源代码管理 · ${x.git?.branch}` : '源代码管理'}
          aria-label="源代码管理"
          aria-pressed={gitOpen}
          onClick={() => setGitOpen((value) => !value)}
        >
          <GitBranch weight="duotone" size={20} />
          {gitSwitched && <span className="git-branch-hint">{x.git?.branch}</span>}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title={
            gitSwitched
              ? '历史版本只读'
              : `在 ${ides.find((ide) => ide.id === prefs.ide)?.name || 'IDE'} 中打开`
          }
          aria-label="在 IDE 中打开"
          disabled={gitSwitched || x.directoryReadOnly}
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
            title={
              gitSwitched
                ? '历史版本只读'
                : `在 ${terminals.find((t) => t.id === prefs.terminal)?.name || '终端'} 中打开`
            }
            aria-label="Open in Terminal"
            disabled={gitSwitched || x.directoryReadOnly}
            onClick={() => terminal()}
            onContextMenu={(event) => {
              event.preventDefault()
              setPicker({ kind: 'terminal', x: event.clientX, y: event.clientY })
            }}
          >
            <TerminalSquare weight="regular" size={20} />
          </Button>
        </div>
        <div className="toolbar-divider" />
        <Button
          variant="ghost"
          size="icon"
          title={split ? '关闭当前面板' : '双面板'}
          aria-label={split ? '关闭当前面板' : '双面板'}
          aria-pressed={split}
          onClick={onToggleSplit}
        >
          <Columns2 size={20} />
        </Button>

        <Button
          variant="ghost"
          size="icon"
          title="状态中心"
          aria-label="状态中心"
          onClick={() => activity.setOpen(!activity.open)}
        >
          <ScrollText size={20} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
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
    </>
  )
}

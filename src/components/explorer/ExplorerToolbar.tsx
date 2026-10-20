import { Choice, Label, Menu } from '@/components/Menus'
import { Button } from '@/components/ui/button'
import type { AppController } from '@/hooks/useAppController'
import type { Group, Sort, View } from '@/lib/types'
import {
  ArrowDownUp,
  ChevronLeft,
  ChevronRight,
  Code2,
  Columns3,
  FolderPlus,
  FolderTree,
  GitBranch,
  Grid2X2,
  Image,
  List,
  PanelLeft,
  Settings2,
  TerminalSquare,
} from 'lucide-react'
import type { ReactNode } from 'react'
const views: { id: View; name: string; icon: ReactNode }[] = [
  { id: 'icons', name: '图标', icon: <Grid2X2 /> },
  { id: 'list', name: '列表', icon: <List /> },
  { id: 'columns', name: '分栏', icon: <Columns3 /> },
  { id: 'gallery', name: '画廊', icon: <Image /> },
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
  control,
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
  openIde,
  terminal,
  setSettings,
}: Pick<
  AppController,
  | 'sidebar'
  | 'control'
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
  | 'openIde'
  | 'terminal'
  | 'setSettings'
>) {
  return (
    <>
      <header className="toolbar" data-tauri-drag-region>
        {!sidebar && (
          <div className="traffic">
            <button title="关闭窗口" className="traffic-close" onClick={() => control('close')} />
            <button
              title="最小化"
              className="traffic-minimize"
              onClick={() => control('minimize')}
            />
            <button
              title="最大化"
              className="traffic-maximize"
              onClick={() => control('toggleMaximize')}
            />
          </div>
        )}
        <Button
          variant="ghost"
          size="icon"
          aria-label="切换侧栏"
          title="切换侧栏"
          onClick={() => setSidebar(!sidebar)}
        >
          <PanelLeft />
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
            <ChevronLeft />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            disabled={!x.canForward}
            title="前进"
            aria-label="前进"
            onClick={x.forward}
          >
            <ChevronRight />
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
        <Menu label="分组方式" icon={<FolderTree />}>
          <Label>分组方式</Label>
          {groupOptions.map(([id, name]) => (
            <Choice key={id} checked={prefs.group === id} onClick={() => update({ group: id })}>
              {name}
            </Choice>
          ))}
        </Menu>
        <Menu label="排序方式" icon={<ArrowDownUp />}>
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
        <Button
          variant="ghost"
          size="icon"
          title={gitSwitched ? '历史版本只读' : `新建文件夹 ${hint('newFolder')}`}
          aria-label="新建文件夹"
          disabled={gitSwitched}
          onClick={() => create(true)}
        >
          <FolderPlus />
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
          <GitBranch />
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
          disabled={gitSwitched}
          onClick={() => openIde()}
        >
          <Code2 />
        </Button>
        <div className="toolbar-divider" />
        <div className="terminal-control">
          <Button
            variant="ghost"
            size="icon"
            title={gitSwitched ? '历史版本只读' : 'Open in Terminal'}
            aria-label="Open in Terminal"
            disabled={gitSwitched}
            onClick={() => terminal()}
          >
            <TerminalSquare />
          </Button>
        </div>
        <Button
          variant="ghost"
          size="icon"
          title="设置"
          aria-label="设置"
          onClick={() => setSettings(true)}
        >
          <Settings2 />
        </Button>
      </header>
    </>
  )
}

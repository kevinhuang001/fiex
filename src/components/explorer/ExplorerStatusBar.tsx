import type { AppController } from '@/hooks/useAppController'
import type { Entry } from '@/lib/types'
import { size } from '@/lib/files'
import { ListChecks, TerminalSquare, X } from 'lucide-react'
export function ExplorerStatusBar({
  x,
  chosenAll,
  selectedAll,
  clipboard,
  setClipboard,
  workspace,
}: Pick<
  AppController,
  'x' | 'chosenAll' | 'selectedAll' | 'clipboard' | 'setClipboard' | 'workspace'
> & { chosenAll: Entry[]; selectedAll: string[] }) {
  return (
    <>
      <footer className="statusbar" data-workspace-drop="true">
        <div className="statusbar-row">
          <span>
            {x.entries.length.toLocaleString()} 个项目
            {selectedAll.length > 0 && ` · 已选择 ${selectedAll.length} 项`}
            {chosenAll.length > 0 &&
              !chosenAll.some((e) => e.isDir) &&
              ` · ${size(chosenAll.reduce((a, e) => a + e.size, 0))}`}
          </span>
          {x.tagFilter && <span>标签：{x.tagFilter}</span>}
          <span>{x.searchStats}</span>
          <div className="toolbar-spacer" />
          {clipboard && (
            <button title="清除剪贴板" onClick={() => setClipboard(null)}>
              {clipboard.moving ? '剪切' : '复制'} {clipboard.paths.length} 项 <X size={11} />
            </button>
          )}
          <button
            className="statusbar-terminal"
            title="打开终端"
            aria-label="打开终端"
            onClick={() =>
              window.dispatchEvent(
                new CustomEvent('fiex:show-terminal-panel', { detail: { cwd: x.path } }),
              )
            }
          >
            <TerminalSquare size={13} />
            终端
          </button>
          <button
            className="workspace-toggle"
            aria-expanded={workspace.open}
            aria-label="工作区"
            onClick={() => workspace.setOpen(!workspace.open)}
          >
            <ListChecks size={13} />
            工作区
            <span className="workspace-count">{workspace.items.length}</span>
          </button>
        </div>
      </footer>
    </>
  )
}

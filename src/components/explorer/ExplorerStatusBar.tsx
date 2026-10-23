import type { AppController } from '@/hooks/useAppController'
import { size } from '@/lib/files'
import { ChevronDown, ChevronUp, ListChecks, X } from 'lucide-react'
export function ExplorerStatusBar({
  x,
  chosen,
  clipboard,
  setClipboard,
  workspace,
}: Pick<AppController, 'x' | 'chosen' | 'clipboard' | 'setClipboard' | 'workspace'>) {
  return (
    <>
      <footer className="statusbar" data-workspace-drop="true">
        <div className="statusbar-row">
          <span>
            {x.entries.length.toLocaleString()} 个项目
            {x.selected.length > 0 && ` · 已选择 ${x.selected.length} 项`}
            {chosen.length > 0 &&
              !chosen.some((e) => e.isDir) &&
              ` · ${size(chosen.reduce((a, e) => a + e.size, 0))}`}
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
            className="workspace-toggle"
            aria-expanded={workspace.open}
            aria-label="工作区"
            onClick={() => workspace.setOpen(!workspace.open)}
          >
            <ListChecks size={13} />
            工作区
            <span className="workspace-count">{workspace.items.length}</span>
            {workspace.open ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
          </button>
        </div>
      </footer>
    </>
  )
}

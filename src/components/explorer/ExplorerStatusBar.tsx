import type { AppController } from '@/hooks/useAppController'
import { size } from '@/lib/files'
import { ChevronDown, ChevronUp, GitBranch, ListChecks, LoaderCircle, X } from 'lucide-react'
export function ExplorerStatusBar({
  busy,
  x,
  chosen,
  gitSwitched,
  clipboard,
  setClipboard,
  workspace,
}: Pick<
  AppController,
  'busy' | 'x' | 'chosen' | 'gitSwitched' | 'clipboard' | 'setClipboard' | 'workspace'
>) {
  return (
    <>
      <footer className="statusbar" data-workspace-drop="true">
        <div className="statusbar-row">
          <span>
            {busy ? (
              <>
                <LoaderCircle className="spin" size={13} />
                {busy}
              </>
            ) : x.searching ? (
              <>
                <LoaderCircle className="spin" size={13} />
                正在搜索…
              </>
            ) : (
              <>
                {x.entries.length.toLocaleString()} 个项目
                {x.selected.length > 0 && ` · 已选择 ${x.selected.length} 项`}
                {chosen.length > 0 &&
                  !chosen.some((e) => e.isDir) &&
                  ` · ${size(chosen.reduce((a, e) => a + e.size, 0))}`}
              </>
            )}
          </span>
          {x.git && (
            <span className="git-status" title={x.git.root}>
              <GitBranch size={12} />
              {x.git.branch}
              {!gitSwitched &&
                Object.keys(x.git.entries).length > 0 &&
                ` · ${Object.keys(x.git.entries).length} 更改`}
            </span>
          )}
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

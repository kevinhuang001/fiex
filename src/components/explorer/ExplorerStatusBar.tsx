import type { AppController } from '@/hooks/useAppController'
import { size } from '@/lib/files'
import {
  ChevronDown,
  ChevronUp,
  Eraser,
  GitBranch,
  ListChecks,
  LoaderCircle,
  X,
} from 'lucide-react'
import { useState } from 'react'
import { FileIcon } from '../FileIcon'
export function ExplorerStatusBar({
  busy,
  x,
  chosen,
  gitSwitched,
  clipboard,
  setClipboard,
  workspace,
  open,
}: Pick<
  AppController,
  'busy' | 'x' | 'chosen' | 'gitSwitched' | 'clipboard' | 'setClipboard' | 'workspace' | 'open'
>) {
  const [workspaceOpen, setWorkspaceOpen] = useState(false)
  return (
    <>
      <footer
        className={`statusbar ${workspaceOpen ? 'workspace-open' : ''}`}
        data-workspace-drop="true"
      >
        {workspaceOpen && (
          <div className="workspace-items">
            {workspace.items.length ? (
              workspace.items.map((entry) => (
                <span className="workspace-chip" key={entry.path}>
                  <button className="workspace-open" title={entry.path} onClick={() => open(entry)}>
                    {entry.isDir ? <ListChecks size={13} /> : <FileIcon entry={entry} size={14} />}
                    <span>{entry.name}</span>
                  </button>
                  <button
                    className="workspace-remove"
                    aria-label={`移出工作区 ${entry.name}`}
                    onClick={() => workspace.remove(entry.path)}
                  >
                    <X size={11} />
                  </button>
                </span>
              ))
            ) : (
              <p className="workspace-empty">把文件拖到这条栏上收纳，不会复制。</p>
            )}
          </div>
        )}
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
          {workspace.items.length > 0 && (
            <button title="清空工作区" aria-label="清空工作区" onClick={workspace.clear}>
              <Eraser size={13} />
            </button>
          )}
          <button
            className="workspace-toggle"
            aria-expanded={workspaceOpen}
            aria-label="工作区"
            onClick={() => setWorkspaceOpen((value) => !value)}
          >
            <ListChecks size={13} />
            工作区
            <span className="workspace-count">{workspace.items.length}</span>
            {workspaceOpen ? <ChevronDown size={12} /> : <ChevronUp size={12} />}
          </button>
        </div>
      </footer>
    </>
  )
}

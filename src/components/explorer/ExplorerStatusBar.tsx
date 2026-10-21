import type { AppController } from '@/hooks/useAppController'
import { size } from '@/lib/files'
import { GitBranch, History, LoaderCircle, X } from 'lucide-react'
import { useActivity } from '@/hooks/useActivity'
export function ExplorerStatusBar({
  busy,
  x,
  chosen,
  gitSwitched,
  clipboard,
  setClipboard,
}: Pick<AppController, 'busy' | 'x' | 'chosen' | 'gitSwitched' | 'clipboard' | 'setClipboard'>) {
  const activity = useActivity()
  return (
    <>
      <footer className="statusbar">
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
        <button
          title="状态中心"
          aria-label="状态中心"
          onClick={() => activity.setOpen(!activity.open)}
        >
          <History size={13} />
        </button>
        {clipboard && (
          <button title="清除剪贴板" onClick={() => setClipboard(null)}>
            {clipboard.moving ? '剪切' : '复制'} {clipboard.paths.length} 项 <X size={11} />
          </button>
        )}
      </footer>
    </>
  )
}

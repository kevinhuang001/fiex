import type { AppController } from '@/hooks/useAppController'
import { Columns2, Folder, Plus, X } from 'lucide-react'
export function ExplorerTabs({
  left,
  panePaths,
  displayName,
  switchWorkspaceTab,
  closeWorkspaceTab,
  newWorkspaceTab,
}: Pick<
  AppController,
  | 'left'
  | 'panePaths'
  | 'displayName'
  | 'switchWorkspaceTab'
  | 'closeWorkspaceTab'
  | 'newWorkspaceTab'
>) {
  return (
    <>
      <div className="tabs" role="tablist" aria-label="标签页">
        {left.tabs.map((path, i) => (
          <div className={`tab ${i === left.tab ? 'active' : ''}`} key={i}>
            <button
              role="tab"
              title={panePaths[i] ? `${path} · ${panePaths[i]}` : path}
              aria-selected={i === left.tab}
              onClick={() => switchWorkspaceTab(i)}
            >
              {panePaths[i] ? <Columns2 size={14} /> : <Folder size={13} />}
              <span>
                {displayName(path)}
                {panePaths[i] ? ` · ${displayName(panePaths[i])}` : ''}
              </span>
            </button>
            {left.tabs.length > 1 && (
              <button aria-label="关闭标签页" onClick={() => closeWorkspaceTab(i)}>
                <X size={12} />
              </button>
            )}
          </div>
        ))}
        <button
          className="icon-button"
          title="新建标签页"
          aria-label="新建标签页"
          onClick={newWorkspaceTab}
        >
          <Plus size={15} />
        </button>
        <div className="tabs-spacer" data-tauri-drag-region />
      </div>
    </>
  )
}

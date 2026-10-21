import type { AppController } from '@/hooks/useAppController'
import { Folder, Plus, X } from 'lucide-react'
export function ExplorerTabs({ x, displayName }: Pick<AppController, 'x' | 'displayName'>) {
  return (
    <>
      <div className="tabs" role="tablist" aria-label="标签页">
        {x.tabs.map((path, i) => (
          <div className={`tab ${i === x.tab ? 'active' : ''}`} key={i}>
            <button role="tab" aria-selected={i === x.tab} onClick={() => x.switchTab(i)}>
              <Folder size={13} />
              <span>{displayName(path)}</span>
            </button>
            {x.tabs.length > 1 && (
              <button aria-label="关闭标签页" onClick={() => x.closeTab(i)}>
                <X size={12} />
              </button>
            )}
          </div>
        ))}
        <button
          className="icon-button"
          title="新建标签页"
          aria-label="新建标签页"
          onClick={() => x.newTab()}
        >
          <Plus size={15} />
        </button>
        <div className="tabs-spacer" data-tauri-drag-region />
      </div>
    </>
  )
}

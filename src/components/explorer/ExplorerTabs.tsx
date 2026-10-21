import { useRef, useState, type PointerEvent } from 'react'
import type { AppController } from '@/hooks/useAppController'
import { Columns2, Folder, Plus, X } from 'lucide-react'
export function ExplorerTabs({
  left,
  panePaths,
  displayName,
  switchWorkspaceTab,
  closeWorkspaceTab,
  newWorkspaceTab,
  moveWorkspaceTab,
}: Pick<
  AppController,
  | 'left'
  | 'panePaths'
  | 'displayName'
  | 'switchWorkspaceTab'
  | 'closeWorkspaceTab'
  | 'newWorkspaceTab'
  | 'moveWorkspaceTab'
>) {
  const [dropIndex, setDropIndex] = useState<number | null>(null)
  const dragged = useRef(false)
  const beginDrag = (event: PointerEvent, from: number) => {
    if (event.button !== 0 || (event.target as Element).closest('[aria-label="关闭标签页"]')) return
    const start = event.clientX
    let to = from
    dragged.current = false
    const move = (next: globalThis.PointerEvent) => {
      if (Math.abs(next.clientX - start) < 5 && !dragged.current) return
      dragged.current = true
      next.preventDefault()
      const target = document
        .elementFromPoint(next.clientX, next.clientY)
        ?.closest<HTMLElement>('[data-tab-index]')
      if (target) {
        to = Number(target.dataset.tabIndex)
        setDropIndex(to)
      }
    }
    const finish = (next: globalThis.PointerEvent) => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', finish)
      window.removeEventListener('pointercancel', finish)
      if (dragged.current && next.type !== 'pointercancel') moveWorkspaceTab(from, to)
      setDropIndex(null)
      setTimeout(() => {
        dragged.current = false
      }, 0)
    }
    window.addEventListener('pointermove', move, { passive: false })
    window.addEventListener('pointerup', finish)
    window.addEventListener('pointercancel', finish)
  }
  return (
    <>
      <div className="tabs" role="tablist" aria-label="标签页">
        {left.tabs.map((path, i) => (
          <div
            className={`tab ${i === left.tab ? 'active' : ''} ${dropIndex === i ? 'tab-drop' : ''}`}
            key={i}
            data-tab-index={i}
            onPointerDown={(event) => beginDrag(event, i)}
          >
            <button
              role="tab"
              title={panePaths[i] ? `${path} · ${panePaths[i]}` : path}
              aria-selected={i === left.tab}
              onClick={() => {
                if (!dragged.current) switchWorkspaceTab(i)
              }}
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

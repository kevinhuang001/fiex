import { Eraser, ListChecks, X } from 'lucide-react'
import type { Entry } from '@/lib/types'
import { FileIcon } from './FileIcon'

/// Collapsible tray of references to files scattered across folders. Dropping
/// an item here never copies it; clicking a chip opens it.
export function WorkspaceBar({
  items,
  open,
  onOpen,
  onRemove,
  onClear,
}: {
  items: Entry[]
  open: boolean
  onOpen: (entry: Entry) => void
  onRemove: (path: string) => void
  onClear: () => void
}) {
  return (
    <section
      className={`workspace-bar ${open ? 'open' : ''}`}
      data-workspace-drop="true"
      aria-label="工作区"
    >
      <header>
        <ListChecks size={14} />
        <span>工作区</span>
        <span className="workspace-count">{items.length}</span>
        <span className="workspace-hint">把文件拖到这里收纳，只记录引用</span>
        {items.length > 0 && (
          <button className="workspace-clear" onClick={onClear} title="清空工作区">
            <Eraser size={13} />
            清空
          </button>
        )}
      </header>
      {open && (
        <div className="workspace-items">
          {items.length ? (
            items.map((entry) => (
              <span className="workspace-chip" key={entry.path}>
                <button className="workspace-open" title={entry.path} onClick={() => onOpen(entry)}>
                  <FileIcon entry={entry} size={30} />
                  <span>{entry.name}</span>
                </button>
                <button
                  className="workspace-remove"
                  aria-label={`移出工作区 ${entry.name}`}
                  onClick={() => onRemove(entry.path)}
                >
                  <X size={11} />
                </button>
              </span>
            ))
          ) : (
            <p className="workspace-empty">从上面的列表把文件拖进来，或拖到这条栏上。</p>
          )}
        </div>
      )}
    </section>
  )
}

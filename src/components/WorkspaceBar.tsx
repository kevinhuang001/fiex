import { FolderOpen, Trash2, X } from 'lucide-react'
import type { Entry } from '@/lib/types'
import { FileIcon } from './FileIcon'

export function WorkspaceBar({
  items,
  open,
  onOpen,
  onRemove,
  onClear,
  onClose,
}: {
  items: Entry[]
  open: boolean
  onOpen: (entry: Entry) => void
  onRemove: (path: string) => void
  onClear: () => void
  onClose: () => void
}) {
  if (!open) return null
  return (
    <section className="workspace-bar" data-workspace-drop="true" aria-label="工作区">
      <header className="workspace-header">
        <FolderOpen size={15} />
        <strong>工作区</strong>
        <span>{items.length}</span>
        <div className="workspace-spacer" />
        {items.length > 0 && (
          <button className="workspace-clear" onClick={onClear} title="清空工作区">
            <Trash2 size={14} /> 清空
          </button>
        )}
        <button className="workspace-close" aria-label="收起工作区" onClick={onClose}>
          <X size={15} />
        </button>
      </header>
      <div className="workspace-items">
        {items.length ? (
          items.map((entry) => (
            <div className="workspace-chip" key={entry.path}>
              <button className="workspace-open" title={entry.path} onClick={() => onOpen(entry)}>
                <FileIcon entry={entry} size={27} />
                <span>{entry.name}</span>
              </button>
              <button
                className="workspace-remove"
                aria-label={`移出工作区 ${entry.name}`}
                onClick={() => onRemove(entry.path)}
              >
                <X size={13} />
              </button>
            </div>
          ))
        ) : (
          <div className="workspace-empty">工作区为空</div>
        )}
      </div>
    </section>
  )
}

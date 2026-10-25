import { useEffect, useRef, useState } from 'react'
import type { Entry } from '@/lib/types'
import { FileIcon } from './FileIcon'

export function WorkspaceBar({
  items,
  open,
  onOpen,
  onRemove,
}: {
  items: Entry[]
  open: boolean
  onOpen: (entry: Entry) => void
  onRemove: (path: string) => void
  onClear: () => void
  onClose: () => void
  onTerminal: () => void
}) {
  const [height, setHeight] = useState(108)
  const panel = useRef<HTMLElement>(null)

  useEffect(() => {
    const handle = panel.current?.querySelector('.workspace-resizer')
    if (!handle) return
    let bottom = 0
    const move = (event: MouseEvent) => {
      const next = bottom - event.clientY
      setHeight(Math.min(Math.max(64, window.innerHeight * 0.6), Math.max(52, next)))
    }
    const up = () => {
      document.removeEventListener('mousemove', move)
      document.removeEventListener('mouseup', up)
    }
    const start = () => {
      bottom = panel.current?.getBoundingClientRect().bottom ?? window.innerHeight
      document.addEventListener('mousemove', move)
      document.addEventListener('mouseup', up)
    }
    handle.addEventListener('mousedown', start)
    return () => {
      handle.removeEventListener('mousedown', start)
      document.removeEventListener('mousemove', move)
      document.removeEventListener('mouseup', up)
    }
  }, [open])

  if (!open) return null
  return (
    <section
      ref={panel}
      className="workspace-bar"
      data-workspace-drop="true"
      aria-label="工作区"
      style={{ height }}
    >
      <div className="workspace-resizer" title="调整工作区高度" />
      <div className="workspace-items">
        {items.length ? (
          items.map((entry) => (
            <div className="workspace-chip" key={entry.path}>
              <button
                className="workspace-open"
                title={`打开 ${entry.name}`}
                onClick={() => onOpen(entry)}
              >
                <FileIcon entry={entry} size={27} />
                <span>{entry.name}</span>
              </button>
              <button
                className="workspace-remove"
                aria-label={`移出工作区 ${entry.name}`}
                onClick={() => onRemove(entry.path)}
              >
                x
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

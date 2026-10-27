import { t } from '@/i18n'
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
  const itemsRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const main = panel.current?.parentElement
    if (!main) return
    main.style.setProperty('--workspace-height', `${height}px`)
    return () => {
      main.style.removeProperty('--workspace-height')
    }
  }, [open, height])

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

  useEffect(() => {
    if (!open) return
    const element = itemsRef.current
    if (!element) return
    const grow = () => {
      const needed = element.scrollHeight + 5
      setHeight((value) =>
        Math.min(Math.max(value, needed), Math.max(52, window.innerHeight * 0.55)),
      )
    }
    grow()
    const observer = new ResizeObserver(grow)
    observer.observe(element)
    return () => observer.disconnect()
  }, [open, items.length])

  if (!open) return null
  return (
    <section
      ref={panel}
      className="workspace-bar"
      data-workspace-drop="true"
      aria-label={t('工作区')}
      style={{ height }}
    >
      <div className="workspace-resizer" title={t('调整工作区高度')} />
      <div className="workspace-items" ref={itemsRef}>
        {items.length ? (
          items.map((entry) => (
            <div className="workspace-chip" key={entry.path}>
              <button
                className="workspace-open"
                title={t('打开 {0}', entry.name)}
                onClick={() => onOpen(entry)}
              >
                <FileIcon entry={entry} size={27} />
                <span>{entry.name}</span>
              </button>
              <button
                className="workspace-remove"
                aria-label={t('移出工作区 {0}', entry.name)}
                onClick={() => onRemove(entry.path)}
              >
                x
              </button>
            </div>
          ))
        ) : (
          <div className="workspace-empty">{t('工作区为空')}</div>
        )}
      </div>
    </section>
  )
}

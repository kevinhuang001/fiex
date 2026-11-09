import { t } from '@/i18n'
import { useEffect, useRef, useState } from 'react'
import { ListMinus } from 'lucide-react'
import { MenuLayer } from '@/components/ui/MenuLayer'
import { useMenuPosition } from '@/lib/menuPosition'
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
  const [context, setContext] = useState<{
    path: string
    name: string
    x: number
    y: number
  } | null>(null)
  const panel = useRef<HTMLElement>(null)
  const itemsRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const menuPosition = useMenuPosition(context?.x ?? 0, context?.y ?? 0, menuRef, Boolean(context))

  useEffect(() => {
    if (!context) return
    const dismiss = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) setContext(null)
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setContext(null)
    }
    document.addEventListener('pointerdown', dismiss)
    window.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('pointerdown', dismiss)
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [context])

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
    let next = 0
    let limit = 0
    const apply = () => {
      const main = panel.current?.parentElement
      if (panel.current) panel.current.style.height = `${next}px`
      // Keep the terminal/pane bottoms anchored while the pointer moves,
      // without rerendering the whole dock on every mouse event.
      if (main) main.style.setProperty('--workspace-height', `${next}px`)
    }
    const move = (event: MouseEvent) => {
      next = Math.min(limit, Math.max(52, bottom - event.clientY))
      apply()
    }
    const up = () => {
      document.removeEventListener('mousemove', move)
      document.removeEventListener('mouseup', up)
      document.body.classList.remove('resizing-workspace')
      setHeight(next)
    }
    const start = () => {
      limit = Math.max(64, window.innerHeight * 0.6)
      bottom = panel.current?.getBoundingClientRect().bottom ?? window.innerHeight
      next = panel.current?.getBoundingClientRect().height ?? 0
      document.body.classList.add('resizing-workspace')
      document.addEventListener('mousemove', move)
      document.addEventListener('mouseup', up)
    }
    handle.addEventListener('mousedown', start)
    return () => {
      handle.removeEventListener('mousedown', start)
      document.removeEventListener('mousemove', move)
      document.removeEventListener('mouseup', up)
      document.body.classList.remove('resizing-workspace')
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
            <div
              className="workspace-chip"
              key={entry.path}
              onContextMenu={(event) => {
                event.preventDefault()
                event.stopPropagation()
                setContext({
                  path: entry.path,
                  name: entry.name,
                  x: event.clientX,
                  y: event.clientY,
                })
              }}
            >
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
      {context && (
        <MenuLayer>
          <div ref={menuRef} role="menu" className="context-menu" style={menuPosition}>
            <button
              role="menuitem"
              className="context-danger"
              onClick={() => {
                onRemove(context.path)
                setContext(null)
              }}
            >
              <ListMinus size={15} />
              {t('移出工作区 {0}', context.name)}
            </button>
          </div>
        </MenuLayer>
      )}
    </section>
  )
}

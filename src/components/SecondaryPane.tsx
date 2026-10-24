import { ArrowUp, ChevronRight, HardDrive, LoaderCircle, Search, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Button } from './ui/button'
import { FileViews } from './browser/FileViews'
import { useExplorer } from '@/hooks/useExplorer'
import { shortcutLabel } from '@/lib/shortcuts'
import type { Entry, Preferences } from '@/lib/types'

/// Second browser pane: an independent explorer that shares preferences. The
/// buttons at the bottom copy or move the other pane's selection into the
/// folder this pane is showing, which is the usual dual-pane workflow.
export function SecondaryPane({
  explorer,
  prefs,
  active,
  onFocus,
  onClose,
  onTransfer,
  onOpen,
  onPreview,
  onMenu,
  dragPaths,
  width,
  onWidthChange,
}: {
  explorer: ReturnType<typeof useExplorer>
  prefs: Preferences
  active: boolean
  onFocus: () => void
  onClose: () => void
  onTransfer: (paths: string[], destination: string, moving: boolean) => void
  onOpen: (entry: Entry) => void
  onPreview: (entry: Entry) => void
  onMenu: (entry: Entry, x: number, y: number) => void
  dragPaths?: string[]
  width: number
  onWidthChange: (width: number) => void
}) {
  const x = explorer
  const [draft, setDraft] = useState(x.path)
  const [editing, setEditing] = useState(false)
  const [top, setTop] = useState<number | null>(null)
  const input = useRef<HTMLInputElement>(null)
  const search = useRef<HTMLInputElement>(null)
  const searchHint = shortcutLabel(prefs.shortcuts, 'search', prefs.shortcutStyle)
  const segments = (() => {
    const parts = x.path.split('/').filter(Boolean)
    return parts.map((name, index) => ({
      name,
      path: (x.path.startsWith('/') ? '/' : '') + parts.slice(0, index + 1).join('/'),
    }))
  })()
  useEffect(() => setDraft(x.path), [x.path])
  // Line the pane up with the main path bar so both rows sit at one height.
  useEffect(() => {
    const main = document.querySelector('.main')
    const bar = document.querySelector('.main > .path-toolbar')
    if (!main || !bar) return
    setTop(bar.getBoundingClientRect().top - main.getBoundingClientRect().top)
  }, [])
  useEffect(() => {
    if (editing) input.current?.select()
  }, [editing])
  return (
    <>
      <div
        className="pane-resizer"
        role="separator"
        aria-label="调整面板宽度"
        style={{ right: `${width}%`, ...(top === null ? {} : { top }) }}
        onPointerDown={(event) => {
          const main = document.querySelector('.main')
          if (!main) return
          const rect = main.getBoundingClientRect()
          const move = (e: PointerEvent) => {
            const percent = 100 - ((e.clientX - rect.left) / rect.width) * 100
            onWidthChange(Math.min(75, Math.max(24, percent)))
          }
          const up = () => {
            window.removeEventListener('pointermove', move)
            window.removeEventListener('pointerup', up)
          }
          window.addEventListener('pointermove', move)
          window.addEventListener('pointerup', up)
          event.preventDefault()
        }}
      />
      <aside
        className={`secondary-pane ${active ? 'pane-active' : ''}`}
        aria-label="第二个面板"
        onPointerDownCapture={onFocus}
        style={{ width: `${width}%`, ...(top === null ? {} : { top }) }}
        data-drop-folder={x.path === 'recent://' ? undefined : x.path}
      >
        <div className="path-toolbar">
          <Button variant="ghost" size="icon" title="上一级" aria-label="上一级" onClick={x.up}>
            <ArrowUp />
          </Button>
          <nav
            className="breadcrumbs"
            aria-label="第二个面板的路径"
            onClick={(event) => {
              if (event.target === event.currentTarget) setEditing(true)
            }}
          >
            {editing ? (
              <form
                className="path-editor"
                onSubmit={(event) => {
                  event.preventDefault()
                  const next = draft.trim()
                  if (next) x.navigate(next)
                  setEditing(false)
                }}
              >
                <input
                  ref={input}
                  className="path-input"
                  aria-label="第二个面板的路径"
                  value={draft}
                  spellCheck={false}
                  onChange={(event) => setDraft(event.target.value)}
                  onBlur={() => setEditing(false)}
                  onKeyDown={(event) => {
                    event.stopPropagation()
                    if (event.key === 'Escape') setEditing(false)
                  }}
                />
              </form>
            ) : (
              <>
                <button
                  onClick={() => setEditing(true)}
                  title="编辑或复制路径"
                  aria-label="编辑路径"
                >
                  <HardDrive size={13} />
                </button>
                {segments.map((segment, index) => (
                  <span key={segment.path} className="crumb">
                    <button
                      className="crumb-button"
                      onClick={() => x.navigate(segment.path)}
                      title={segment.path}
                    >
                      {segment.name}
                    </button>
                    {index < segments.length - 1 && (
                      <ChevronRight size={12} className="crumb-sep" />
                    )}
                  </span>
                ))}
              </>
            )}
          </nav>
          <div className="search-box">
            <Search size={15} />
            <input
              ref={search}
              aria-label="搜索第二个面板"
              placeholder="搜索此文件夹"
              value={x.query}
              onChange={(event) => x.setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key !== 'Escape') return
                event.stopPropagation()
                x.setQuery('')
                event.currentTarget.blur()
              }}
            />
            {x.searching ? (
              <LoaderCircle className="spin" size={14} />
            ) : x.query ? (
              <button className="icon-button" aria-label="清除搜索" onClick={() => x.setQuery('')}>
                <X size={13} />
              </button>
            ) : (
              <kbd>{searchHint}</kbd>
            )}
          </div>
          <button
            className="icon-button pane-close-secondary"
            title="关闭面板"
            aria-label="关闭面板"
            onClick={onClose}
          >
            <X size={14} />
          </button>
        </div>
        <FileViews
          explorer={x}
          prefs={prefs}
          onOpen={onOpen}
          onPreview={onPreview}
          onMenu={onMenu}
          onDrop={(paths, destination, moving) => onTransfer(paths, destination, moving)}
          onBookmark={() => {}}
          onWorkspace={() => {}}
          dragPaths={dragPaths}
          readOnly={x.directoryReadOnly}
        />
      </aside>
    </>
  )
}

import { useEffect, useRef, useState } from 'react'
import { FileViews } from './browser/FileViews'
import { PathToolbar } from './explorer/PathToolbar'
import { useExplorer } from '@/hooks/useExplorer'
import { pathCrumbs } from '@/lib/files'
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
  displayName,
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
  displayName: (path: string) => string
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
        <PathToolbar
          x={x}
          crumbs={pathCrumbs(x.path, displayName)}
          editing={editing}
          draft={draft}
          setDraft={setDraft}
          startEdit={() => {
            setDraft(x.path)
            setEditing(true)
          }}
          submit={(event) => {
            event.preventDefault()
            const next = draft.trim()
            if (next) x.navigate(next)
            setEditing(false)
          }}
          stopEdit={() => setEditing(false)}
          pathInput={input}
          searchInput={search}
          searchHint={searchHint}
          ariaLabel="第二个面板的路径"
          searchLabel="搜索第二个面板"
          close={{
            label: '关闭面板',
            title: '关闭面板',
            className: 'pane-close-secondary',
            onClose,
          }}
        />
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

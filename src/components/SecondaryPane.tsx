import { ArrowUp, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { FileViews } from './browser/FileViews'
import { useExplorer } from '@/hooks/useExplorer'
import type { Entry, Preferences } from '@/lib/types'

/// Second browser pane: an independent explorer that shares preferences. The
/// buttons at the bottom copy or move the other pane's selection into the
/// folder this pane is showing, which is the usual dual-pane workflow.
export function SecondaryPane({
  prefs,
  start,
  onClose,
  onTransfer,
}: {
  prefs: Preferences
  start: string
  onClose: () => void
  onTransfer: (paths: string[], destination: string, moving: boolean) => void
}) {
  const x = useExplorer(prefs, start)
  const [draft, setDraft] = useState(x.path)
  useEffect(() => setDraft(x.path), [x.path])
  const open = (entry: Entry) => {
    if (entry.isDir) x.navigate(entry.path)
  }
  return (
    <aside
      className="secondary-pane"
      aria-label="第二个面板"
      data-drop-folder={x.path === 'recent://' ? undefined : x.path}
    >
      <header>
        <button className="icon-button" title="上一级" aria-label="上一级" onClick={x.up}>
          <ArrowUp size={14} />
        </button>
        <input
          className="secondary-path"
          aria-label="第二个面板的路径"
          title={x.path}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== 'Enter') return
            const next = draft.trim()
            if (next && next !== x.path) x.navigate(next)
            event.currentTarget.blur()
          }}
        />
        <button className="icon-button" title="关闭面板" aria-label="关闭面板" onClick={onClose}>
          <X size={14} />
        </button>
      </header>
      <FileViews
        explorer={x}
        prefs={prefs}
        onOpen={open}
        onPreview={() => {}}
        onMenu={() => {}}
        onDrop={(paths, destination, moving) => onTransfer(paths, destination, moving)}
        onBookmark={() => {}}
        onWorkspace={() => {}}
        readOnly
      />
    </aside>
  )
}

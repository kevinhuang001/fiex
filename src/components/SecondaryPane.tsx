import { ArrowUp, X } from 'lucide-react'
import { FileViews } from './browser/FileViews'
import { useExplorer } from '@/hooks/useExplorer'
import { basename } from '@/lib/files'
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
        <span className="secondary-path" title={x.path}>
          {x.path === 'recent://' ? '最近使用' : basename(x.path) || x.path}
        </span>
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

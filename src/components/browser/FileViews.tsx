import { useEffect, useMemo, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import {
  ChevronRight,
  CloudCheck,
  CloudDownload,
  Files,
  FolderOpen,
  LoaderCircle,
  SearchX,
} from 'lucide-react'
import { FileIcon } from '../FileIcon'
import { BrowserColumn } from './BrowserColumn'
import { VirtualFileGrid } from './VirtualFileGrid'
import { InlineRename } from './InlineRename'
import { TagDots } from '../TagDots'
import { asset, call, native } from '@/lib/api'
import { startFileDrag } from '@/lib/drag'
import { date, groups, isImage, kind, parent, size, sortEntries } from '@/lib/files'
import type { Entry, Listing, Preferences } from '@/lib/types'
import type { Explorer } from '@/hooks/useExplorer'
import type { CSSProperties } from 'react'

// macOS stops reporting pointer events once the cursor leaves the window while a
// button is held, so hand the drag to the OS a few pixels before the boundary.
const EDGE = 4
interface Props {
  explorer: Explorer
  prefs: Preferences
  onOpen: (e: Entry) => void
  onPreview: (e: Entry) => void
  onMenu: (e: Entry, x: number, y: number) => void
  onDrop: (paths: string[], dest: string, moving: boolean) => void
  onBookmark: (paths: string[]) => void
  onWorkspace: (paths: string[]) => void
  onExternalDrag?: (result: 'Dropped' | 'Cancelled') => void
  readOnly?: boolean
  trashPath?: string | null
}

type DropTarget =
  | { kind: 'folder'; path: string; element: HTMLElement }
  | { kind: 'bookmark'; element: HTMLElement }
  | { kind: 'workspace'; element: HTMLElement }

function dropTargetAt(x: number, y: number): DropTarget | null {
  const element = document.elementFromPoint(x, y)
  const folder = element?.closest('[data-drop-folder]') as HTMLElement | null
  if (folder?.dataset.dropFolder)
    return { kind: 'folder', path: folder.dataset.dropFolder, element: folder }
  const bookmark = element?.closest('[data-bookmark-drop]') as HTMLElement | null
  if (bookmark) return { kind: 'bookmark', element: bookmark }
  const workspace = element?.closest('[data-workspace-drop]') as HTMLElement | null
  if (workspace) return { kind: 'workspace', element: workspace }
  return null
}

function clearDropHighlight() {
  document
    .querySelectorAll('.drop-target')
    .forEach((element) => element.classList.remove('drop-target'))
  document
    .querySelectorAll('.bookmark-drop')
    .forEach((element) => element.classList.remove('bookmark-drop'))
  document
    .querySelectorAll('.workspace-drop')
    .forEach((element) => element.classList.remove('workspace-drop'))
}

function SyncBadge({ sync }: { sync: Entry['sync'] }) {
  if (!sync) return null
  return sync === 'synced' ? (
    <span className="sync-badge synced" title="已同步">
      <CloudCheck size={11} />
      <span>已同步</span>
    </span>
  ) : (
    <span className="sync-badge cloud" title="仅云端，需要连接">
      <CloudDownload size={11} />
      <span>仅云端</span>
    </span>
  )
}

function NameCell({
  explorer,
  entry,
  className,
}: {
  explorer: Explorer
  entry: Entry
  className: string
}) {
  if (explorer.renaming === entry.path)
    return (
      <InlineRename
        entry={entry}
        className={className}
        onCommit={(name) => explorer.renameTo(entry.path, name)}
        onCancel={explorer.cancelRename}
      />
    )
  return (
    <span className={className} title={entry.name}>
      {entry.name}
    </span>
  )
}

export function FileViews(props: Props) {
  const { explorer: x, prefs, onOpen, onPreview, onMenu, onDrop, onBookmark, onWorkspace } = props
  const readOnly = Boolean(props.readOnly)
  const inTrash = Boolean(props.trashPath && x.path === props.trashPath)
  const entries = useMemo(
    () => sortEntries(x.entries, prefs.sort, prefs.descending),
    [x.entries, prefs.sort, prefs.descending],
  )
  const selectedPaths = useMemo(() => new Set(x.selected), [x.selected])
  // The window is zoomed from the font-size preference, so the tile geometry
  // stays at the reference size and scales with the rest of the interface.
  const iconSize = 88
  const anchor = useRef<string | null>(null)
  const suppressClick = useRef(false)
  const container = useRef<HTMLDivElement>(null)
  const [band, setBand] = useState<{
    left: number
    top: number
    width: number
    height: number
  } | null>(null)
  const bandStart = useRef<{ x: number; y: number; base: string[]; additive: boolean } | null>(null)
  const bandDragged = useRef(false)
  const startBand = (event: React.PointerEvent) => {
    if (event.button !== 0 || !container.current) return
    // Begin only from empty space: rows, tiles, inputs and buttons keep their
    // own drag/click behaviour.
    if ((event.target as Element).closest('[data-path],button,input,a')) return
    const additive = event.metaKey || event.ctrlKey || event.shiftKey
    bandStart.current = {
      x: event.clientX,
      y: event.clientY,
      base: additive ? [...x.selected] : [],
      additive,
    }
    bandDragged.current = false
    event.currentTarget.setPointerCapture(event.pointerId)
  }
  const moveBand = (event: React.PointerEvent) => {
    const start = bandStart.current
    if (!start || !container.current) return
    const left = Math.min(start.x, event.clientX)
    const top = Math.min(start.y, event.clientY)
    const width = Math.abs(event.clientX - start.x)
    const height = Math.abs(event.clientY - start.y)
    if (width > 3 || height > 3) bandDragged.current = true
    setBand({ left, top, width, height })
    const bandRect = { left, top, right: left + width, bottom: top + height }
    const picked: string[] = []
    container.current.querySelectorAll<HTMLElement>('[data-path]').forEach((element) => {
      const rect = element.getBoundingClientRect()
      const hit =
        rect.left < bandRect.right &&
        rect.right > bandRect.left &&
        rect.top < bandRect.bottom &&
        rect.bottom > bandRect.top
      if (hit && element.dataset.path) picked.push(element.dataset.path)
    })
    const next = start.additive ? [...new Set([...start.base, ...picked])] : picked
    x.setSelected(next)
  }
  const endBand = () => {
    bandStart.current = null
    setBand(null)
  }
  const [ghost, setGhost] = useState<{ paths: string[]; x: number; y: number } | null>(null)
  const beginPointerDrag = (event: React.PointerEvent, entry: Entry) => {
    if (event.button !== 0 || x.renaming || readOnly) return
    if ((event.target as Element).closest('input,button,a')) return
    const paths = selectedPaths.has(entry.path) ? [...x.selected] : [entry.path]
    if (!event.metaKey && !event.ctrlKey && !event.shiftKey && !selectedPaths.has(entry.path)) {
      x.setSelected([entry.path])
    }
    const startX = event.clientX
    const startY = event.clientY
    // Keep pointer events flowing once the cursor crosses the window edge:
    // without capture macOS/Windows stop reporting moves before the drag can
    // be handed over to the native drag session.
    const captureTarget = event.currentTarget as Element
    const pointerId = event.pointerId
    try {
      captureTarget.setPointerCapture(pointerId)
    } catch {
      // Older webviews may not support pointer capture; the edge check below
      // still handles the common case.
    }
    const releaseCapture = () => {
      try {
        if (captureTarget.hasPointerCapture?.(pointerId))
          captureTarget.releasePointerCapture(pointerId)
      } catch {
        // Capture is already gone.
      }
    }
    let active = false
    let nativeStarted = false
    const cleanup = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', finish)
      window.removeEventListener('pointercancel', finish)
      document.removeEventListener('mouseleave', leave)
      releaseCapture()
      document.body.classList.remove('dragging-file')
      clearDropHighlight()
      setGhost(null)
    }
    const startNative = (copyRequested = false) => {
      if (!active || nativeStarted) return
      nativeStarted = true
      cleanup()
      if (!native) return
      void call('ci_log', {
        message: `startNative active=${active} inner=${window.innerWidth}x${window.innerHeight} paths=${JSON.stringify(paths)}`,
      }).catch(() => {})
      void startFileDrag(
        paths,
        (result) => {
          props.onExternalDrag?.(result)
          if (result === 'Dropped') x.reload()
        },
        copyRequested,
      ).catch((error) => {
        x.setError(String(error))
      })
    }
    const leave = (event: MouseEvent) => startNative(event.altKey || event.ctrlKey)
    const move = (moveEvent: PointerEvent) => {
      if (!active) {
        if (Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY) < 5) return
        active = true
        document.body.classList.add('dragging-file')
      }
      if (
        native &&
        (moveEvent.clientX <= EDGE ||
          moveEvent.clientY <= EDGE ||
          moveEvent.clientX >= window.innerWidth - EDGE ||
          moveEvent.clientY >= window.innerHeight - EDGE)
      ) {
        startNative(moveEvent.altKey || moveEvent.ctrlKey)
        return
      }
      moveEvent.preventDefault()
      setGhost({ paths, x: moveEvent.clientX, y: moveEvent.clientY })
      clearDropHighlight()
      const target = dropTargetAt(moveEvent.clientX, moveEvent.clientY)
      if (target?.kind === 'folder') target.element.classList.add('drop-target')
      else if (target?.kind === 'bookmark') target.element.classList.add('bookmark-drop')
      else if (target?.kind === 'workspace') target.element.classList.add('workspace-drop')
    }
    const finish = (upEvent: PointerEvent) => {
      cleanup()
      if (!active || nativeStarted) return
      suppressClick.current = true
      setTimeout(() => {
        suppressClick.current = false
      }, 0)
      const target = dropTargetAt(upEvent.clientX, upEvent.clientY)
      if (target?.kind === 'folder' && !paths.includes(target.path))
        onDrop(paths, target.path, !(upEvent.altKey || upEvent.ctrlKey))
      else if (target?.kind === 'bookmark') onBookmark(paths)
      else if (target?.kind === 'workspace') onWorkspace(paths)
    }
    window.addEventListener('pointermove', move, { passive: false })
    window.addEventListener('pointerup', finish)
    window.addEventListener('pointercancel', finish)
    document.addEventListener('mouseleave', leave)
  }
  const select = (e: Entry, event: React.MouseEvent) => {
    if (suppressClick.current) {
      event.preventDefault()
      event.stopPropagation()
      return
    }
    ;(event.currentTarget.closest('[role="grid"]') as HTMLElement | null)?.focus({
      preventScroll: true,
    })
    if (event.shiftKey && anchor.current) {
      const a = entries.findIndex((x) => x.path === anchor.current)
      const b = entries.indexOf(e)
      x.setSelected(entries.slice(Math.min(a, b), Math.max(a, b) + 1).map((x) => x.path))
    } else if (event.metaKey || event.ctrlKey) {
      x.setSelected((s) => (s.includes(e.path) ? s.filter((p) => p !== e.path) : [...s, e.path]))
      anchor.current = e.path
    } else {
      x.setSelected([e.path])
      anchor.current = e.path
    }
  }
  const shared = (e: Entry) => {
    const renaming = x.renaming === e.path
    return {
      onClick: (event: React.MouseEvent) => select(e, event),
      onDoubleClick: () => {
        if (!renaming) onOpen(e)
      },
      onContextMenu: (event: React.MouseEvent) => {
        event.preventDefault()
        if (!selectedPaths.has(e.path)) x.setSelected([e.path])
        onMenu(e, event.clientX, event.clientY)
      },
      onPointerDown: (event: React.PointerEvent) => beginPointerDrag(event, e),
      'data-path': e.path,
      'data-drop-folder': e.isDir ? e.path : undefined,
      'aria-selected': selectedPaths.has(e.path),
      tabIndex: -1,
    }
  }
  const flat = useMemo(
    () =>
      groups(entries, prefs.group).flatMap(([heading, files]) => [
        ...(heading ? [{ heading, entry: null }] : []),
        ...files.map((entry) => ({ heading: '', entry })),
      ]),
    [entries, prefs.group],
  )
  const virtual = useVirtualizer({
    count: flat.length,
    getScrollElement: () => container.current,
    estimateSize: () => 38,
    overscan: 12,
  })
  useEffect(() => {
    if (!x.renaming) return
    const index = flat.findIndex((row) => row.entry?.path === x.renaming)
    if (prefs.view === 'list' && index >= 0) virtual.scrollToIndex(index)
    const frame = requestAnimationFrame(() =>
      container.current
        ?.querySelector(`[data-path="${CSS.escape(x.renaming!)}"]`)
        ?.scrollIntoView({ block: 'nearest' }),
    )
    return () => cancelAnimationFrame(frame)
  }, [x.renaming, flat, prefs.view, virtual])
  const onKeyDown = (event: React.KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'a') {
      event.preventDefault()
      x.setSelected(entries.map((entry) => entry.path))
      return
    }
    if (x.renaming || event.metaKey || event.ctrlKey || event.altKey) return
    if (
      !['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key) ||
      !entries.length
    )
      return
    event.preventDefault()
    const last = entries.findIndex((e) => e.path === x.selected.at(-1))
    let next =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? entries.length - 1
          : Math.max(
              0,
              Math.min(
                entries.length - 1,
                last + (event.key === 'ArrowUp' || event.key === 'ArrowLeft' ? -1 : 1),
              ),
            )
    if (last < 0 && event.key !== 'End' && event.key !== 'Home') next = 0
    const e = entries[next]
    x.setSelected(event.shiftKey ? [...new Set([...x.selected, e.path])] : [e.path])
    const row = flat.findIndex((r) => r.entry?.path === e.path)
    if (prefs.view === 'list') virtual.scrollToIndex(row)
    else
      requestAnimationFrame(() =>
        container.current
          ?.querySelector(`[data-path="${CSS.escape(e.path)}"]`)
          ?.scrollIntoView({ block: 'nearest' }),
      )
  }
  if (prefs.view === 'columns' && !x.query)
    return <Columns {...props} shared={shared} onKeyDown={onKeyDown} />
  return (
    <div
      className="file-area"
      ref={container}
      role="grid"
      aria-label="文件"
      tabIndex={0}
      onKeyDown={onKeyDown}
      onPointerDown={startBand}
      onPointerMove={moveBand}
      onPointerUp={endBand}
      onPointerCancel={endBand}
      onClick={(e) => {
        // A finished rubber-band gesture must not collapse the selection.
        if (bandDragged.current) {
          bandDragged.current = false
          return
        }
        if (e.target === e.currentTarget) x.setSelected([])
      }}
    >
      {band && (
        <div
          className="selection-band"
          style={{ left: band.left, top: band.top, width: band.width, height: band.height }}
        />
      )}
      {entries.length === 0 && !x.loading && !x.error && (
        <div className="empty-state">
          {x.searching ? (
            <>
              <LoaderCircle className="spin" />
              <h3>正在搜索…</h3>
            </>
          ) : (
            <>
              {x.query ? <SearchX /> : <FolderOpen />}
              <h3>
                {x.query
                  ? '没有找到匹配的文件'
                  : x.path === 'recent://'
                    ? '还没有最近打开的文件'
                    : inTrash
                      ? '废纸篓是空的'
                      : '文件夹是空的'}
              </h3>
            </>
          )}
        </div>
      )}
      {prefs.view === 'list' && entries.length > 0 ? (
        <>
          <div className="list-header">
            <span>名称</span>
            <span>修改时间</span>
            <span>大小</span>
            <span>种类</span>
          </div>
          <div style={{ height: virtual.getTotalSize(), position: 'relative' }}>
            {virtual.getVirtualItems().map((row) => {
              const item = flat[row.index]
              return (
                <div
                  key={item.entry?.path || item.heading}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    height: row.size,
                    transform: `translateY(${row.start}px)`,
                  }}
                >
                  {item.entry ? (
                    <div
                      role="row"
                      className={`file-row ${selectedPaths.has(item.entry.path) ? 'selected' : ''}`}
                      {...shared(item.entry)}
                    >
                      <span className="file-name">
                        <FileIcon entry={item.entry} size={25} />
                        <NameCell explorer={x} entry={item.entry} className="file-label" />
                        <TagDots tags={item.entry.tags} />
                        <SyncBadge sync={item.entry.sync} />
                        {item.entry.readonly && x.renaming !== item.entry.path && (
                          <small>只读</small>
                        )}
                      </span>
                      <span>{date(item.entry.modified)}</span>
                      <span>{item.entry.isDir ? '—' : size(item.entry.size)}</span>
                      <span>{kind(item.entry)}</span>
                    </div>
                  ) : (
                    <div className="group-heading">{item.heading}</div>
                  )}
                </div>
              )
            })}
          </div>
        </>
      ) : (
        <>
          {prefs.view === 'gallery' && x.selected.length > 0 && (
            <div
              className="gallery-preview"
              onDoubleClick={() => onPreview(entries.find((e) => e.path === x.selected[0])!)}
            >
              {(() => {
                const e = entries.find((e) => e.path === x.selected[0])
                return e ? (
                  <>
                    <Thumbnail entry={e} size={140} />
                    <strong>{e.name}</strong>
                    <span>
                      {kind(e)} · {e.isDir ? '文件夹' : size(e.size)}
                    </span>
                    <button onClick={() => onPreview(e)}>空格快速查看</button>
                  </>
                ) : null
              })()}
            </div>
          )}
          {prefs.view === 'icons' && entries.length > 200 ? (
            <VirtualFileGrid
              entries={entries}
              group={prefs.group}
              tileSize={Math.max(96, iconSize + 38)}
              rowHeight={iconSize + 76}
              scrollRef={container}
              target={x.renaming || x.selected.at(-1) || null}
              render={(e) => (
                <div
                  role="gridcell"
                  className={`file-tile ${selectedPaths.has(e.path) ? 'selected' : ''}`}
                  key={e.path}
                  {...shared(e)}
                >
                  <span className="tile-icon">
                    <Thumbnail entry={e} size={iconSize} />
                  </span>
                  <NameCell explorer={x} entry={e} className="tile-label" />
                  <TagDots tags={e.tags} />
                  <SyncBadge sync={e.sync} />
                </div>
              )}
            />
          ) : (
            groups(entries, prefs.group).map(([heading, files]) => (
              <section key={heading}>
                {heading && <div className="group-heading">{heading}</div>}
                <div
                  className={prefs.view === 'gallery' ? 'gallery-strip' : 'icon-grid'}
                  style={
                    prefs.view !== 'gallery'
                      ? ({
                          '--tile-size': `${Math.max(96, iconSize + 38)}px`,
                          '--icon-size': `${iconSize}px`,
                          '--label-size': '11px',
                        } as CSSProperties)
                      : undefined
                  }
                  onClick={(event) => {
                    if (event.target === event.currentTarget) x.setSelected([])
                  }}
                >
                  {files.map((e) => (
                    <div
                      role="gridcell"
                      className={`file-tile ${selectedPaths.has(e.path) ? 'selected' : ''}`}
                      key={e.path}
                      {...shared(e)}
                    >
                      <span className="tile-icon">
                        <Thumbnail entry={e} size={prefs.view === 'gallery' ? 64 : iconSize} />
                      </span>
                      <NameCell explorer={x} entry={e} className="tile-label" />
                      <TagDots tags={e.tags} />
                      <SyncBadge sync={e.sync} />
                    </div>
                  ))}
                </div>
              </section>
            ))
          )}
        </>
      )}
      {ghost && (
        <div className="drag-ghost" style={{ left: ghost.x + 14, top: ghost.y + 14 }}>
          <Files size={14} />
          {ghost.paths.length} 个项目
        </div>
      )}
    </div>
  )
}

function Thumbnail({ entry: e, size: s }: { entry: Entry; size: number }) {
  const [failed, setFailed] = useState(false)
  useEffect(() => setFailed(false), [e.path])
  if (isImage(e) && !failed && asset(e.path))
    return (
      <img
        className="thumbnail"
        style={{ width: s, height: s }}
        src={asset(e.path)}
        alt=""
        loading="lazy"
        draggable={false}
        onError={() => setFailed(true)}
      />
    )
  return <FileIcon entry={e} size={s} />
}

function Columns({
  explorer: x,
  prefs,
  onOpen,
  onPreview,
  shared,
  onKeyDown,
}: Props & {
  shared: (e: Entry) => Record<string, unknown>
  onKeyDown: (e: React.KeyboardEvent) => void
}) {
  const selectedPaths = useMemo(() => new Set(x.selected), [x.selected])
  const [left, setLeft] = useState<Entry[]>([])
  const [right, setRight] = useState<Entry[]>([])
  const [error, setError] = useState('')
  const selected = x.entries.find((e) => e.path === x.selected[0])
  useEffect(() => {
    let active = true
    void call<Listing>('list_directory', {
      path: parent(x.path),
      hidden: prefs.hidden,
      ignores: prefs.displayIgnore,
    })
      .then((l) => {
        if (active) setLeft(l.entries)
      })
      .catch(() => setLeft([]))
    return () => {
      active = false
    }
  }, [x.path, prefs.hidden, prefs.displayIgnore])
  useEffect(() => {
    let active = true
    setRight([])
    setError('')
    if (selected?.isDir)
      void call<Listing>('list_directory', {
        path: selected.path,
        hidden: prefs.hidden,
        ignores: prefs.displayIgnore,
      })
        .then((l) => {
          if (active) setRight(l.entries)
        })
        .catch((e) => {
          if (active) setError(String(e))
        })
    return () => {
      active = false
    }
  }, [selected?.path, selected?.isDir, prefs.hidden, prefs.displayIgnore])
  const column = (files: Entry[], current: boolean) => (
    <BrowserColumn
      entries={files}
      prefs={prefs}
      target={current ? x.selected.at(-1) : x.path}
      render={(e) => (
        <div
          key={e.path}
          className={`column-row ${selectedPaths.has(e.path) || e.path === x.path ? 'selected' : ''}`}
          data-drop-folder={e.isDir ? e.path : undefined}
          {...(current
            ? shared(e)
            : {
                onClick: () => (e.isDir ? x.navigate(e.path) : onPreview(e)),
                onDoubleClick: () => onOpen(e),
              })}
        >
          <FileIcon entry={e} size={24} />
          <NameCell explorer={x} entry={e} className="column-label" />
          <TagDots tags={e.tags} size={6} />
          <SyncBadge sync={e.sync} />
          {e.isDir && <ChevronRight size={13} />}
        </div>
      )}
    />
  )
  return (
    <div className="columns-view" role="grid" aria-label="文件" tabIndex={0} onKeyDown={onKeyDown}>
      {column(left, false)}
      {column(x.entries, true)}
      {selected?.isDir ? (
        <>
          {column(right, false)}
          {error && <p className="error">{error}</p>}
        </>
      ) : selected ? (
        <div className="column-detail" onDoubleClick={() => onPreview(selected)}>
          <Thumbnail entry={selected} size={100} />
          <strong>{selected.name}</strong>
          <span>
            {kind(selected)} · {size(selected.size)}
          </span>
          <button onClick={() => onPreview(selected)}>快速查看</button>
        </div>
      ) : (
        <div className="column-detail muted" />
      )}
    </div>
  )
}

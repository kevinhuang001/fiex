import { useEffect, useMemo, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { ChevronRight, FolderOpen, SearchX } from 'lucide-react'
import { FileIcon } from '../FileIcon'
import { InlineRename } from './InlineRename'
import { asset, call } from '@/lib/api'
import { date, groups, isImage, kind, parent, size, sortEntries } from '@/lib/files'
import type { Entry, Listing, Preferences } from '@/lib/types'
import type { Explorer } from '@/hooks/useExplorer'
import type { CSSProperties } from 'react'

interface Props {
  explorer: Explorer
  prefs: Preferences
  onOpen: (e: Entry) => void
  onPreview: (e: Entry) => void
  onMenu: (e: Entry, x: number, y: number) => void
  onDrop: (paths: string[], dest: string, moving: boolean) => void
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
  const { explorer: x, prefs, onOpen, onPreview, onMenu, onDrop } = props
  const entries = useMemo(
    () => sortEntries(x.entries, prefs.sort, prefs.descending),
    [x.entries, prefs.sort, prefs.descending],
  )
  const anchor = useRef<string | null>(null)
  const container = useRef<HTMLDivElement>(null)
  const select = (e: Entry, event: React.MouseEvent) => {
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
        if (!x.selected.includes(e.path)) x.setSelected([e.path])
        onMenu(e, event.clientX, event.clientY)
      },
      draggable: !renaming,
      onDragStart: (event: React.DragEvent) => {
        event.dataTransfer.setData(
          'application/x-fiex',
          JSON.stringify(x.selected.includes(e.path) ? x.selected : [e.path]),
        )
        event.dataTransfer.effectAllowed = 'copyMove'
      },
      onDragOver: (event: React.DragEvent) => {
        if (e.isDir) {
          event.preventDefault()
          event.dataTransfer.dropEffect = event.altKey || event.ctrlKey ? 'copy' : 'move'
          event.currentTarget.classList.add('drop-target')
        }
      },
      onDragLeave: (event: React.DragEvent) => event.currentTarget.classList.remove('drop-target'),
      onDrop: (event: React.DragEvent) => {
        if (e.isDir) {
          event.preventDefault()
          event.stopPropagation()
          event.currentTarget.classList.remove('drop-target')
          try {
            onDrop(
              JSON.parse(event.dataTransfer.getData('application/x-fiex')),
              e.path,
              !(event.altKey || event.ctrlKey),
            )
          } catch {
            /* Native OS drop is handled by Tauri. */
          }
        }
      },
      'data-path': e.path,
      'aria-selected': x.selected.includes(e.path),
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
    if (last < 0) next = 0
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
  const dropBackground = (event: React.DragEvent) => {
    event.preventDefault()
    try {
      onDrop(
        JSON.parse(event.dataTransfer.getData('application/x-fiex')),
        x.path,
        !(event.altKey || event.ctrlKey),
      )
    } catch {
      /* Ignore unrelated browser drags. */
    }
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
      onDragOver={(e) => e.preventDefault()}
      onDrop={dropBackground}
      onClick={(e) => {
        if (e.target === e.currentTarget) x.setSelected([])
      }}
    >
      {entries.length === 0 && !x.loading && !x.error && (
        <div className="empty-state">
          {x.query ? <SearchX /> : <FolderOpen />}
          <h3>{x.query ? '没有找到匹配的文件' : '文件夹是空的'}</h3>
          <p>{x.query ? '试试其他名称，或调整搜索范围。' : '拖入文件，或新建文件夹。'}</p>
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
                      className={`file-row ${x.selected.includes(item.entry.path) ? 'selected' : ''}`}
                      {...shared(item.entry)}
                    >
                      <span className="file-name">
                        <FileIcon entry={item.entry} size={25} />
                        <NameCell explorer={x} entry={item.entry} className="file-label" />
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
          {groups(entries, prefs.group).map(([heading, files]) => (
            <section key={heading}>
              {heading && <div className="group-heading">{heading}</div>}
              <div
                className={prefs.view === 'gallery' ? 'gallery-strip' : 'icon-grid'}
                style={
                  prefs.view !== 'gallery'
                    ? ({
                        '--tile-size': `${Math.max(96, prefs.iconSize + 38)}px`,
                        '--icon-size': `${prefs.iconSize}px`,
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
                    className={`file-tile ${x.selected.includes(e.path) ? 'selected' : ''}`}
                    key={e.path}
                    {...shared(e)}
                  >
                    <span className="tile-icon">
                      <Thumbnail entry={e} size={prefs.view === 'gallery' ? 64 : prefs.iconSize} />
                    </span>
                    <NameCell explorer={x} entry={e} className="tile-label" />
                  </div>
                ))}
              </div>
            </section>
          ))}
        </>
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
  const [left, setLeft] = useState<Entry[]>([])
  const [right, setRight] = useState<Entry[]>([])
  const [error, setError] = useState('')
  const selected = x.entries.find((e) => e.path === x.selected[0])
  useEffect(() => {
    let active = true
    void call<Listing>('list_directory', { path: parent(x.path), hidden: prefs.hidden })
      .then((l) => {
        if (active) setLeft(l.entries)
      })
      .catch(() => setLeft([]))
    return () => {
      active = false
    }
  }, [x.path, prefs.hidden])
  useEffect(() => {
    let active = true
    setRight([])
    setError('')
    if (selected?.isDir)
      void call<Listing>('list_directory', { path: selected.path, hidden: prefs.hidden })
        .then((l) => {
          if (active) setRight(l.entries)
        })
        .catch((e) => {
          if (active) setError(String(e))
        })
    return () => {
      active = false
    }
  }, [selected?.path, selected?.isDir, prefs.hidden])
  const column = (files: Entry[], current: boolean) => (
    <div className="browser-column">
      {sortEntries(files, prefs.sort, prefs.descending).map((e) => (
        <div
          key={e.path}
          className={`column-row ${x.selected.includes(e.path) || e.path === x.path ? 'selected' : ''}`}
          {...(current
            ? shared(e)
            : {
                onClick: () => (e.isDir ? x.navigate(e.path) : onPreview(e)),
                onDoubleClick: () => onOpen(e),
              })}
        >
          <FileIcon entry={e} size={24} />
          <NameCell explorer={x} entry={e} className="column-label" />
          {e.isDir && <ChevronRight size={13} />}
        </div>
      ))}
    </div>
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
        <div className="column-detail muted">选择文件查看预览</div>
      )}
    </div>
  )
}

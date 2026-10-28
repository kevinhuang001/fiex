import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useRef } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Files, FolderOpen, LoaderCircle, SearchX } from 'lucide-react'
import { ColumnsView } from './views/ColumnsView'
import { GridView } from './views/GridView'
import { ListView } from './views/ListView'
import { useFileDrag, useSelectionBand } from './views/useSelectionBand'
import { groups, sortEntries } from '@/lib/files'
import type { Entry } from '@/lib/types'
import type { FlatRow } from './views/ListView'
import type { Props, SharedRow } from './views/shared'
import type { MouseEvent } from 'react'

export function FileViews(props: Props) {
  const {
    explorer: x,
    prefs,
    updatePrefs,
    onOpen,
    onPreview,
    onMenu,
    onDrop,
    onBookmark,
    onWorkspace,
    dragPaths,
  } = props
  const readOnly = Boolean(props.readOnly)
  const inTrash = Boolean(props.trashPath && x.path === props.trashPath)
  const entries = useMemo(
    () => sortEntries(x.entries, prefs.sort, prefs.descending),
    [x.entries, prefs.sort, prefs.descending, getLocale()],
  )
  const selectedPaths = useMemo(() => new Set(x.selected), [x.selected, getLocale()])
  // The window is zoomed from the font-size preference, so the tile geometry
  // stays at the reference size and scales with the rest of the interface.
  const iconSize = 64
  const anchor = useRef<string | null>(null)
  const container = useRef<HTMLDivElement>(null)
  const band = useSelectionBand({ container, selected: x.selected, setSelected: x.setSelected })
  const drag = useFileDrag({
    explorer: x,
    selectedPaths,
    dragPaths,
    readOnly,
    onDrop,
    onBookmark,
    onWorkspace,
    onExternalDrag: props.onExternalDrag,
  })
  const select = (e: Entry, event: MouseEvent) => {
    if (drag.suppressClick.current) {
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
  const shared: SharedRow = (e) => {
    const renaming = x.renaming === e.path
    return {
      onClick: (event: MouseEvent) => select(e, event),
      onDoubleClick: () => {
        if (!renaming) onOpen(e)
      },
      onContextMenu: (event: MouseEvent) => {
        event.preventDefault()
        if (!selectedPaths.has(e.path)) x.setSelected([e.path])
        onMenu(e, event.clientX, event.clientY)
      },
      onPointerDown: (event) => drag.beginPointerDrag(event, e),
      'data-path': e.path,
      'data-drop-folder': e.isDir ? e.path : undefined,
      'aria-selected': selectedPaths.has(e.path),
      tabIndex: -1,
    }
  }
  const flat = useMemo<FlatRow[]>(
    () =>
      groups(entries, prefs.group).flatMap(([heading, files]) => [
        ...(heading ? [{ heading, entry: null }] : []),
        ...files.map((entry) => ({ heading: '', entry })),
      ]),
    [entries, prefs.group, getLocale()],
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
    return <ColumnsView {...props} shared={shared} onKeyDown={onKeyDown} />
  return (
    <div
      className="file-area"
      ref={container}
      role="grid"
      aria-label={t('文件')}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onPointerDown={band.startBand}
      onPointerMove={band.moveBand}
      onPointerUp={band.endBand}
      onPointerCancel={band.endBand}
      onClick={band.handleContainerClick}
    >
      {band.band && (
        <div
          className="selection-band"
          style={{
            left: band.band.left,
            top: band.band.top,
            width: band.band.width,
            height: band.band.height,
          }}
        />
      )}
      {entries.length === 0 && !x.loading && !x.error && (
        <div className="empty-state">
          {x.searching ? (
            <>
              <LoaderCircle className="spin" />
              <h3>{t('正在搜索…')}</h3>
            </>
          ) : (
            <>
              {x.query ? <SearchX /> : <FolderOpen />}
              <h3>
                {x.query
                  ? t('没有找到匹配的文件')
                  : x.path === 'recent://'
                    ? t('还没有最近打开的文件')
                    : inTrash
                      ? t('废纸篓是空的')
                      : t('文件夹是空的')}
              </h3>
            </>
          )}
        </div>
      )}
      {prefs.view === 'list' && entries.length > 0 ? (
        <ListView
          explorer={x}
          flat={flat}
          columns={prefs.listColumns}
          onColumnsChange={(listColumns) => updatePrefs({ listColumns })}
          rows={virtual.getVirtualItems()}
          totalSize={virtual.getTotalSize()}
          selectedPaths={selectedPaths}
          shared={shared}
        />
      ) : (
        <GridView
          explorer={x}
          prefs={prefs}
          entries={entries}
          iconSize={iconSize}
          selectedPaths={selectedPaths}
          shared={shared}
          container={container}
          onPreview={onPreview}
        />
      )}
      {drag.ghost && (
        <div className="drag-ghost" style={{ left: drag.ghost.x + 14, top: drag.ghost.y + 14 }}>
          <Files size={14} />
          {drag.ghost.paths.length} {t('个项目')}
        </div>
      )}
    </div>
  )
}

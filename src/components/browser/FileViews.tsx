import { useDirectoryColumns } from '@/features/explorer/hooks/useDirectoryColumns'
import { t, getLocale } from '@/i18n'
import { isWindows } from '@/lib/platform'
import { useEffect, useMemo, useRef } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Files, FolderOpen, LoaderCircle, SearchX } from 'lucide-react'
import { ColumnsView } from './views/ColumnsView'
import { GridView } from './views/GridView'
import { ListView } from './views/ListView'
import { useFileDrag, useSelectionBand } from './views/useSelectionBand'
import { groups, nextSort, sortEntries } from '@/lib/files'
import {
  moveVisiblePathSelection,
  selectAllVisiblePaths,
  selectVisiblePath,
} from '@/features/explorer/model/fileSelection'
import type { Entry, Sort } from '@/lib/types'
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
    onBlankMenu,
    onDrop,
    onBookmark,
    onWorkspace,
    dragPaths,
  } = props
  const [columns, updateColumns] = useDirectoryColumns(x.path, prefs.listColumns)
  const inTrash = Boolean(props.trashPath && x.path === props.trashPath)
  const entries = useMemo(
    () => sortEntries(x.entries, prefs.sort, prefs.descending),
    [x.entries, prefs.sort, prefs.descending, getLocale()],
  )
  const selectedPaths = useMemo(() => new Set(x.selected), [x.selected, getLocale()])
  const visiblePaths = useMemo(() => entries.map((entry) => entry.path), [entries])
  // Header clicks reuse the preference the toolbar sort menu and every view
  // already read, so the header indicator, the list rows and search results
  // cannot drift apart. A fresh column starts ascending, the active one flips.
  const sortBy = (sort: Sort) => updatePrefs(nextSort(prefs.sort, prefs.descending, sort))
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
    const selection = selectVisiblePath({
      visiblePaths,
      selectedPaths: x.selected,
      clickedPath: e.path,
      anchorPath: anchor.current,
      extendRange: event.shiftKey,
      additive: event.metaKey || event.ctrlKey,
    })
    x.setSelected(selection.selectedPaths)
    anchor.current = selection.anchorPath
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
    const target = x.renaming ?? x.selected.at(-1)
    if (!target) return
    const index = flat.findIndex((row) => row.entry?.path === target)
    if (prefs.view === 'list' && index >= 0) virtual.scrollToIndex(index)
    const frame = requestAnimationFrame(() =>
      container.current
        ?.querySelector(`[data-path="${CSS.escape(target)}"]`)
        ?.scrollIntoView({ block: 'nearest' }),
    )
    return () => cancelAnimationFrame(frame)
  }, [x.renaming, x.selected, flat, prefs.view, virtual])
  const onKeyDown = (event: React.KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'a') {
      event.preventDefault()
      x.setSelected(selectAllVisiblePaths(visiblePaths))
      return
    }
    const selection = moveVisiblePathSelection({
      visiblePaths,
      selectedPaths: x.selected,
      key: event.key,
      extendSelection: event.shiftKey,
      disabled: Boolean(x.renaming || event.metaKey || event.ctrlKey || event.altKey),
    })
    if (!selection) return
    event.preventDefault()
    x.setSelected(selection.selectedPaths)
    const row = flat.findIndex((r) => r.entry?.path === selection.focusedPath)
    if (prefs.view === 'list') virtual.scrollToIndex(row)
    else
      requestAnimationFrame(() =>
        container.current
          ?.querySelector(`[data-path="${CSS.escape(selection.focusedPath)}"]`)
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
      onContextMenu={(event) => {
        if (event.defaultPrevented) return
        event.preventDefault()
        onBlankMenu(event.clientX, event.clientY)
      }}
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
                      ? t(isWindows() ? '回收站是空的' : '废纸篓是空的')
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
          columns={columns}
          onColumnsChange={updateColumns}
          sort={prefs.sort}
          descending={prefs.descending}
          onSort={sortBy}
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

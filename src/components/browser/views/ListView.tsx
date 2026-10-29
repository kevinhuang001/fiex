import { t } from '@/i18n'
import { useRef, useState } from 'react'
import { ArrowDown, ArrowUp } from 'lucide-react'
import { FileIcon } from '../../FileIcon'
import { DecoratedValue, NameCell, rowVars } from './shared'
import { date, kind, size } from '@/lib/files'
import { LIST_COLUMNS_DEFAULT, listGridTemplate, resizeListColumn } from '@/lib/listColumns'
import type { ListColumnKey, ListColumns } from '@/lib/listColumns'
import type { Entry, Sort } from '@/lib/types'
import type { Explorer } from '@/hooks/useExplorer'
import type { SharedRow } from './shared'
import type { PointerEvent as ReactPointerEvent } from 'react'
import type { VirtualItem } from '@tanstack/react-virtual'

export interface FlatRow {
  heading: string
  entry: Entry | null
}

/** Virtualized list with a fixed header row and draggable column dividers. */
export function ListView({
  explorer,
  flat,
  rows,
  totalSize,
  selectedPaths,
  shared,
  columns,
  onColumnsChange,
  sort,
  descending,
  onSort,
}: {
  explorer: Explorer
  flat: FlatRow[]
  rows: VirtualItem[]
  totalSize: number
  selectedPaths: Set<string>
  shared: SharedRow
  columns: ListColumns
  onColumnsChange: (columns: ListColumns) => void
  sort: Sort
  descending: boolean
  onSort: (sort: Sort) => void
}) {
  // A drag previews locally and commits once on release, so moving the divider
  // never writes preferences on every pointer event.
  const [draft, setDraft] = useState<ListColumns | null>(null)
  const drag = useRef<{ startX: number; columns: ListColumns; key: ListColumnKey } | null>(null)
  const widths = draft ?? columns
  const template = listGridTemplate(widths)
  const beginDrag = (key: ListColumnKey, event: ReactPointerEvent<HTMLElement>) => {
    event.preventDefault()
    event.stopPropagation()
    drag.current = { startX: event.clientX, columns: widths, key }
    document.body.classList.add('col-resizing')
    event.currentTarget.setPointerCapture(event.pointerId)
  }
  const moveDrag = (event: ReactPointerEvent<HTMLElement>) => {
    const state = drag.current
    if (!state) return
    setDraft(resizeListColumn(state.columns, state.key, event.clientX - state.startX))
  }
  const endDrag = () => {
    if (!drag.current) return
    drag.current = null
    document.body.classList.remove('col-resizing')
    if (
      draft &&
      (draft.name !== columns.name ||
        draft.modified !== columns.modified ||
        draft.size !== columns.size ||
        draft.kind !== columns.kind)
    )
      onColumnsChange(draft)
    setDraft(null)
  }
  const resetColumn = (key: ListColumnKey) => {
    setDraft(null)
    onColumnsChange({ ...widths, [key]: LIST_COLUMNS_DEFAULT[key] })
  }
  const resizer = (key: ListColumnKey) => (
    <i
      role="separator"
      aria-orientation="vertical"
      aria-label={t('拖动调整列宽')}
      className="col-resizer"
      onPointerDown={(event) => beginDrag(key, event)}
      onPointerMove={moveDrag}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onDoubleClick={() => resetColumn(key)}
    />
  )
  // The header and the rows share one sort state: FileViews sorts the entries
  // with the same `sort`/`descending` pair this header paints its indicator
  // from. A new column starts ascending; clicking the active column flips it.
  const ariaSort = (key: Sort) =>
    sort === key ? (descending ? 'descending' : 'ascending') : 'none'
  const sortButton = (key: Sort, label: string) => {
    const active = sort === key
    const hint = t('按{0}排序', label)
    return (
      <button
        type="button"
        className="list-sort"
        onClick={() => onSort(key)}
        aria-label={hint}
        title={hint}
      >
        {label}
        {active &&
          (descending ? (
            <ArrowDown size={11} aria-hidden="true" />
          ) : (
            <ArrowUp size={11} aria-hidden="true" />
          ))}
      </button>
    )
  }
  return (
    <>
      <div className="list-header" style={{ gridTemplateColumns: template }} role="row">
        <span role="columnheader" aria-sort={ariaSort('name')}>
          {sortButton('name', t('名称'))}
        </span>
        <span className="list-col" role="columnheader" aria-sort={ariaSort('modified')}>
          {resizer('modified')}
          {sortButton('modified', t('修改时间'))}
        </span>
        <span className="list-col" role="columnheader" aria-sort={ariaSort('size')}>
          {resizer('size')}
          {sortButton('size', t('大小'))}
        </span>
        <span className="list-col" role="columnheader" aria-sort={ariaSort('kind')}>
          {resizer('kind')}
          {sortButton('kind', t('种类'))}
        </span>
      </div>
      <div style={{ height: totalSize, position: 'relative' }}>
        {rows.map((row) => {
          const item = flat[row.index]
          const decoration = item.entry ? explorer.decorations?.[item.entry.path] : undefined
          const rowDecor = rowVars(decoration)
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
                  className={`file-row ${selectedPaths.has(item.entry.path) ? 'selected' : ''}${rowDecor.className}`}
                  style={{ ...rowDecor.style, gridTemplateColumns: template }}
                  {...shared(item.entry)}
                >
                  <span className="file-name">
                    <FileIcon entry={item.entry} size={25} />
                    <NameCell explorer={explorer} entry={item.entry} className="file-label" />
                  </span>
                  <DecoratedValue decoration={decoration} target="modified">
                    {date(item.entry.modified)}
                  </DecoratedValue>
                  <DecoratedValue decoration={decoration} target="size">
                    {item.entry.isDir ? '—' : size(item.entry.size)}
                  </DecoratedValue>
                  <DecoratedValue decoration={decoration} target="kind">
                    {kind(item.entry)}
                  </DecoratedValue>
                </div>
              ) : (
                <div className="group-heading">{item.heading}</div>
              )}
            </div>
          )
        })}
      </div>
    </>
  )
}

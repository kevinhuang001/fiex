import {
  LIST_COLUMNS_DEFAULT,
  LIST_COLUMNS_MAX,
  LIST_COLUMNS_MIN,
  LIST_NAME_MIN,
  type ListColumnKey,
  type ListColumns,
} from '@/lib/listColumns'

function clamp(value: number, min: number): number {
  return Math.max(min, Math.min(LIST_COLUMNS_MAX, Math.round(value)))
}

/** CSS grid template shared by the header and every row. Stored widths remain
 * the preferred layout; narrow panes proportionally compress every column. */
export function listGridTemplate(columns: ListColumns, availableWidth = 0): string {
  const preferred = [
    columns.name ?? LIST_COLUMNS_DEFAULT.name ?? LIST_NAME_MIN,
    columns.modified,
    columns.size,
    columns.kind,
  ]
  const fixedSpace = 24 + 14 * (preferred.length - 1)
  const total = preferred.reduce((sum, width) => sum + width, 0)
  if (availableWidth <= fixedSpace || total <= availableWidth - fixedSpace)
    return `minmax(${preferred[0]}px, 1fr) ${preferred
      .slice(1)
      .map((width) => `${width}px`)
      .join(' ')}`
  const scale = (availableWidth - fixedSpace) / total
  const [name, ...fixed] = preferred.map((width) => Math.max(1, Math.floor(width * scale)))
  return `minmax(${name}px, 1fr) ${fixed.map((width) => `${width}px`).join(' ')}`
}

/** Applies one divider drag while preserving the neighboring fixed column's
 * width and preventing either column from becoming unusably small. */
export function resizeListColumn(
  columns: ListColumns,
  column: ListColumnKey,
  dx: number,
): ListColumns {
  const next = { ...columns }
  if (column === 'name') {
    next.name = clamp(
      (columns.name ?? LIST_COLUMNS_DEFAULT.name ?? LIST_NAME_MIN) + dx,
      LIST_NAME_MIN,
    )
    return next
  }
  const neighbour: ListColumnKey | null =
    column === 'size' ? 'modified' : column === 'kind' ? 'size' : null
  const wanted = Math.round((columns[column] ?? LIST_COLUMNS_MIN[column] ?? LIST_NAME_MIN) - dx)
  if (!neighbour) {
    next[column] = clamp(wanted, LIST_COLUMNS_MIN[column] ?? LIST_NAME_MIN)
    return next
  }
  const total = (columns[column] ?? 0) + (columns[neighbour] ?? 0)
  const max = Math.min(LIST_COLUMNS_MAX, total - (LIST_COLUMNS_MIN[neighbour] ?? LIST_NAME_MIN))
  next[column] = Math.max(LIST_COLUMNS_MIN[column] ?? LIST_NAME_MIN, Math.min(wanted, max))
  next[neighbour] = total - next[column]
  return next
}

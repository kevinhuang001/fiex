/** List-view column widths.
 *
 * The name column stays flexible and absorbs the remaining space; the three
 * trailing columns keep explicit pixel widths so dragging a divider moves just
 * that divider. Widths are persisted with the other preferences. */
export interface ListColumns {
  /** User's minimum width; grows to fill a wide pane. */
  name?: number
  modified: number
  size: number
  kind: number
}

export type ListColumnKey = keyof ListColumns

export const LIST_COLUMNS_DEFAULT: ListColumns = { name: 260, modified: 155, size: 75, kind: 108 }
export const LIST_COLUMNS_MIN: ListColumns = { name: 140, modified: 110, size: 60, kind: 70 }
/** No column may swallow the whole window by accident. */
export const LIST_COLUMNS_MAX = 640
/** The flexible name column never shrinks below this. */
export const LIST_NAME_MIN = 140

function clamp(value: number, min: number): number {
  return Math.max(min, Math.min(LIST_COLUMNS_MAX, Math.round(value)))
}

/** Stored values may be absent, partial or hand-edited; every width falls back
 * to its default and is clamped to a usable range. */
export function normalizeListColumns(value: unknown): ListColumns {
  const source =
    value && typeof value === 'object' ? (value as Partial<Record<string, unknown>>) : {}
  const read = (key: ListColumnKey) => {
    const raw = source[key]
    return typeof raw === 'number' && Number.isFinite(raw)
      ? clamp(raw, LIST_COLUMNS_MIN[key] ?? LIST_NAME_MIN)
      : LIST_COLUMNS_DEFAULT[key]
  }
  return {
    name: read('name'),
    modified: read('modified')!,
    size: read('size')!,
    kind: read('kind')!,
  }
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

/** Applies one divider drag. `dx` is the pointer movement in pixels and the
 * column names the divider's right-hand column: the divider belongs to that
 * column, so the pointer drags its width while the neighbour takes the
 * difference. The name column is flexible, so its divider only changes
 * `modified` and lets the name column absorb the rest. */
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
    // The name column is flexible, so its divider only moves this column.
    next[column] = clamp(wanted, LIST_COLUMNS_MIN[column] ?? LIST_NAME_MIN)
    return next
  }
  // The pair keeps its combined width: one column only grows by what the other
  // gives up, so the divider stays under the pointer.
  const total = (columns[column] ?? 0) + (columns[neighbour] ?? 0)
  const max = Math.min(LIST_COLUMNS_MAX, total - (LIST_COLUMNS_MIN[neighbour] ?? LIST_NAME_MIN))
  next[column] = Math.max(LIST_COLUMNS_MIN[column] ?? LIST_NAME_MIN, Math.min(wanted, max))
  next[neighbour] = total - next[column]
  return next
}

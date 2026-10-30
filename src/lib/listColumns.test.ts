import { describe, expect, it } from 'vitest'
import {
  LIST_COLUMNS_DEFAULT,
  LIST_COLUMNS_MAX,
  LIST_COLUMNS_MIN,
  LIST_NAME_MIN,
  listGridTemplate,
  normalizeListColumns,
  resizeListColumn,
} from './listColumns'

describe('normalizeListColumns', () => {
  it('falls back to the defaults for missing or broken values', () => {
    expect(normalizeListColumns(undefined)).toEqual(LIST_COLUMNS_DEFAULT)
    expect(normalizeListColumns('list')).toEqual(LIST_COLUMNS_DEFAULT)
    expect(normalizeListColumns({ modified: 'wide', size: null })).toEqual(LIST_COLUMNS_DEFAULT)
  })

  it('keeps valid widths and clamps the rest', () => {
    expect(normalizeListColumns({ modified: 200, size: 90, kind: 120 })).toEqual({
      name: LIST_COLUMNS_DEFAULT.name,
      modified: 200,
      size: 90,
      kind: 120,
    })
    expect(normalizeListColumns({ modified: 4, size: 9999, kind: 71 })).toEqual({
      name: LIST_COLUMNS_DEFAULT.name,
      modified: LIST_COLUMNS_MIN.modified,
      size: LIST_COLUMNS_MAX,
      kind: 71,
    })
  })

  it('rounds fractional widths', () => {
    expect(normalizeListColumns({ modified: 200.6, size: 90.2, kind: 120.5 })).toEqual({
      name: LIST_COLUMNS_DEFAULT.name,
      modified: 201,
      size: 90,
      kind: 121,
    })
  })
})

describe('listGridTemplate', () => {
  it('keeps the name column flexible and pins the trailing widths', () => {
    expect(listGridTemplate({ modified: 155, size: 75, kind: 108 })).toBe(
      `minmax(${LIST_COLUMNS_DEFAULT.name}px, 1fr) 155px 75px 108px`,
    )
  })
})

describe('resizeListColumn', () => {
  it('supports a user-defined name column minimum', () => {
    expect(resizeListColumn({ ...LIST_COLUMNS_DEFAULT }, 'name', 45).name).toBe(305)
    expect(resizeListColumn({ ...LIST_COLUMNS_DEFAULT }, 'name', -9999).name).toBe(LIST_NAME_MIN)
  })

  it('drags the divider of a fixed column and moves the neighbour the other way', () => {
    const columns = { modified: 155, size: 75, kind: 108 }
    // Dragging the size divider right shrinks size and grows modified.
    expect(resizeListColumn(columns, 'size', 10)).toEqual({ modified: 165, size: 65, kind: 108 })
    // Dragging it left widens size and shrinks modified.
    expect(resizeListColumn(columns, 'size', -20)).toEqual({ modified: 135, size: 95, kind: 108 })
    // The pair keeps its combined width either way.
    expect(resizeListColumn(columns, 'kind', 25)).toEqual({ modified: 155, size: 100, kind: 83 })
  })

  it('only changes modified at the name divider so the name column absorbs it', () => {
    expect(resizeListColumn({ modified: 155, size: 75, kind: 108 }, 'modified', 30)).toEqual({
      modified: 125,
      size: 75,
      kind: 108,
    })
  })

  it('never goes below the minimum width', () => {
    const dragged = resizeListColumn({ modified: 155, size: 75, kind: 108 }, 'size', 500)
    expect(dragged.size).toBe(LIST_COLUMNS_MIN.size)
    expect(dragged.modified).toBe(155 + (75 - LIST_COLUMNS_MIN.size))
  })

  it('stops the neighbour from collapsing too', () => {
    const dragged = resizeListColumn({ modified: 110, size: 75, kind: 108 }, 'size', -500)
    expect(dragged.modified).toBe(LIST_COLUMNS_MIN.modified)
    // The neighbour cannot grow past what the pair had together.
    expect(dragged.size).toBe(75)
  })
})

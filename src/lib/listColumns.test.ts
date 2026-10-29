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

const widths = { ...LIST_COLUMNS_DEFAULT }

describe('normalizeListColumns', () => {
  it('falls back to defaults for missing or broken values', () => {
    expect(normalizeListColumns(undefined)).toEqual(widths)
    expect(normalizeListColumns('list')).toEqual(widths)
    expect(normalizeListColumns({ modified: 'wide', size: null })).toEqual(widths)
  })

  it('keeps valid widths, including the name field, and clamps the rest', () => {
    expect(normalizeListColumns({ name: 270, modified: 200, size: 90, kind: 120 })).toEqual({
      name: 270,
      modified: 200,
      size: 90,
      kind: 120,
    })
    expect(normalizeListColumns({ name: 40, modified: 4, size: 9999, kind: 71 })).toEqual({
      name: LIST_COLUMNS_MIN.name,
      modified: LIST_COLUMNS_MIN.modified,
      size: LIST_COLUMNS_MAX,
      kind: 71,
    })
  })

  it('rounds fractional widths', () => {
    expect(normalizeListColumns({ name: 280.8, modified: 200.6, size: 90.2, kind: 120.5 })).toEqual({
      name: 281,
      modified: 201,
      size: 90,
      kind: 121,
    })
  })
})

describe('listGridTemplate', () => {
  it('uses saved name minimum and pins the trailing widths', () => {
    expect(listGridTemplate(widths)).toBe(
      `minmax(${LIST_COLUMNS_DEFAULT.name}px, 1fr) 155px 75px 108px`,
    )
    expect(LIST_NAME_MIN).toBe(LIST_COLUMNS_MIN.name)
  })
})

describe('resizeListColumn', () => {
  it('drags fixed column dividers and preserves the pair width', () => {
    expect(resizeListColumn(widths, 'size', 10)).toEqual({
      ...widths,
      modified: 165,
      size: 65,
    })
    expect(resizeListColumn(widths, 'size', -20)).toEqual({
      ...widths,
      modified: 135,
      size: 95,
    })
    expect(resizeListColumn(widths, 'kind', 25)).toEqual({
      ...widths,
      size: 100,
      kind: 83,
    })
  })

  it('lets the name column absorb dragging the modified divider', () => {
    expect(resizeListColumn(widths, 'modified', 30)).toEqual({
      ...widths,
      name: widths.name + 30,
      modified: 125,
    })
  })

  it('supports changing the name width itself', () => {
    expect(resizeListColumn(widths, 'name', 30).name).toBe(widths.name - 30)
  })

  it('never goes below the minimum width', () => {
    const dragged = resizeListColumn(widths, 'size', 500)
    expect(dragged.size).toBe(LIST_COLUMNS_MIN.size)
    expect(dragged.modified).toBe(155 + (75 - LIST_COLUMNS_MIN.size))
  })

  it('stops the neighbour from collapsing', () => {
    const dragged = resizeListColumn(
      { ...widths, modified: 110 },
      'size',
      -500,
    )
    expect(dragged.modified).toBe(LIST_COLUMNS_MIN.modified)
    expect(dragged.size).toBe(75)
  })
})

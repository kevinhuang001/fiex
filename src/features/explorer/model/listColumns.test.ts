import { describe, expect, it } from 'vitest'
import { LIST_COLUMNS_DEFAULT, LIST_COLUMNS_MIN, LIST_NAME_MIN } from '@/lib/listColumns'
import { listGridTemplate, resizeListColumn } from './listColumns'

describe('listGridTemplate', () => {
  it('keeps saved widths when the pane is wide enough', () => {
    expect(listGridTemplate({ modified: 155, size: 75, kind: 108 })).toBe(
      `minmax(${LIST_COLUMNS_DEFAULT.name}px, 1fr) 155px 75px 108px`,
    )
  })

  it('compresses all columns to fit a narrow split pane', () => {
    expect(listGridTemplate(LIST_COLUMNS_DEFAULT, 556)).toBe('minmax(213px, 1fr) 127px 61px 88px')
  })
})

describe('resizeListColumn', () => {
  it('supports a user-defined name column minimum', () => {
    expect(resizeListColumn({ ...LIST_COLUMNS_DEFAULT }, 'name', 45).name).toBe(305)
    expect(resizeListColumn({ ...LIST_COLUMNS_DEFAULT }, 'name', -9999).name).toBe(LIST_NAME_MIN)
  })

  it('drags a fixed divider while its neighboring column moves in the other direction', () => {
    const columns = { modified: 155, size: 75, kind: 108 }
    expect(resizeListColumn(columns, 'size', 10)).toEqual({ modified: 165, size: 65, kind: 108 })
    expect(resizeListColumn(columns, 'size', -20)).toEqual({ modified: 135, size: 95, kind: 108 })
    expect(resizeListColumn(columns, 'kind', 25)).toEqual({ modified: 155, size: 100, kind: 83 })
  })

  it('lets the flexible name column absorb the modified-column divider movement', () => {
    expect(resizeListColumn({ modified: 155, size: 75, kind: 108 }, 'modified', 30)).toEqual({
      modified: 125,
      size: 75,
      kind: 108,
    })
  })

  it('keeps both columns above their minimum widths', () => {
    const dragged = resizeListColumn({ modified: 155, size: 75, kind: 108 }, 'size', 500)
    expect(dragged.size).toBe(LIST_COLUMNS_MIN.size)
    expect(dragged.modified).toBe(155 + (75 - LIST_COLUMNS_MIN.size))
    const constrained = resizeListColumn({ modified: 110, size: 75, kind: 108 }, 'size', -500)
    expect(constrained.modified).toBe(LIST_COLUMNS_MIN.modified)
    expect(constrained.size).toBe(75)
  })
})

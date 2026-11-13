import { describe, expect, it } from 'vitest'
import {
  LIST_COLUMNS_DEFAULT,
  LIST_COLUMNS_MAX,
  LIST_COLUMNS_MIN,
  normalizeListColumns,
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

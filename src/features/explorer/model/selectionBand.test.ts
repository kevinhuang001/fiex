import { describe, expect, it } from 'vitest'
import {
  isSelectionBandDragged,
  mergeBandSelection,
  rectanglesOverlap,
  selectionBandRectangle,
} from './selectionBand'

describe('selectionBand', () => {
  it('normalizes a selection rectangle when dragging in any direction', () => {
    expect(selectionBandRectangle({ x: 20, y: 30 }, { x: 5, y: 9 })).toEqual({
      left: 5,
      top: 9,
      right: 20,
      bottom: 30,
      width: 15,
      height: 21,
    })
  })

  it('distinguishes pointer jitter from a completed band gesture', () => {
    expect(isSelectionBandDragged(selectionBandRectangle({ x: 10, y: 10 }, { x: 13, y: 13 }))).toBe(
      false,
    )
    expect(isSelectionBandDragged(selectionBandRectangle({ x: 10, y: 10 }, { x: 10, y: 14 }))).toBe(
      true,
    )
  })

  it('selects only positive-area intersections and excludes edge contact', () => {
    const band = selectionBandRectangle({ x: 0, y: 0 }, { x: 20, y: 20 })

    expect(rectanglesOverlap(band, { left: 19, top: 19, right: 24, bottom: 24 })).toBe(true)
    expect(rectanglesOverlap(band, { left: 20, top: 4, right: 28, bottom: 12 })).toBe(false)
  })

  it('merges additive picks once while replacement selection uses only hits', () => {
    expect(mergeBandSelection(['/a', '/b'], ['/b', '/c'], true)).toEqual(['/a', '/b', '/c'])
    expect(mergeBandSelection(['/a', '/b'], ['/b', '/c'], false)).toEqual(['/b', '/c'])
  })
})

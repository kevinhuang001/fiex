import { describe, expect, it } from 'vitest'
import {
  moveItem,
  reorderGap,
  reorderShift,
  reorderTargetIndex,
  type ReorderBox,
} from './sidebarReorder'

const boxes: ReorderBox[] = [10, 30, 50, 70].map((center) => ({
  top: center - 5,
  left: 0,
  width: 100,
  height: 10,
  center,
}))

describe('sidebar reorder model', () => {
  it('moves one item to the requested final index', () => {
    expect(moveItem(['a', 'b', 'c', 'd'], 1, 3)).toEqual(['a', 'c', 'd', 'b'])
  })

  it('measures the empty slot between rows', () => {
    expect(reorderGap(boxes)).toBe(10)
    expect(reorderGap(boxes.slice(0, 1))).toBe(0)
  })

  it('computes the final insertion index while excluding the dragged row', () => {
    expect(reorderTargetIndex(60, boxes, 1)).toBe(2)
    expect(reorderTargetIndex(5, boxes, 1)).toBe(0)
  })

  it('shifts only the rows crossed by the dragged item', () => {
    const drag = {
      kind: 'section' as const,
      from: 1,
      to: 3,
      left: 0,
      top: 0,
      width: 100,
      height: 20,
      gap: 4,
      settling: false,
    }
    expect(reorderShift(drag, 'section', 2)).toBe('translate3d(0, -24px, 0)')
    expect(reorderShift(drag, 'section', 1)).toBeUndefined()
    expect(reorderShift(drag, 'place', 2)).toBeUndefined()
  })
})

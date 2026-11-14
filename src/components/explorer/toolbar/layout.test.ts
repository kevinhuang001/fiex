import { describe, expect, it } from 'vitest'
import { orderedToolbarKeys, toolbarItemStyle } from './layout'

describe('orderedToolbarKeys', () => {
  it('ignores unknown saved keys and appends newly available defaults', () => {
    expect(orderedToolbarKeys(['sort', 'removed', 'group'], ['group', 'sort', 'refresh'])).toEqual([
      'sort',
      'group',
      'refresh',
    ])
  })
})

describe('toolbarItemStyle', () => {
  it('keeps a dragged item above its siblings and applies its pointer offset', () => {
    expect(toolbarItemStyle({ drag: { from: 2, x: 18 }, shift: () => undefined }, 2)).toEqual({
      transform: 'translateX(18px)',
      zIndex: 2,
      position: 'relative',
    })
  })

  it('uses sibling displacement for moved slots and leaves unaffected items alone', () => {
    const order = {
      drag: { from: 1, x: 0 },
      shift: (index: number) => (index === 2 ? 'translateX(-40px)' : undefined),
    }
    expect(toolbarItemStyle(order, 2)).toEqual({ transform: 'translateX(-40px)' })
    expect(toolbarItemStyle(order, 0)).toBeUndefined()
  })
})

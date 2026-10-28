import { describe, expect, it } from 'vitest'
import { placeMenu } from './menuPosition'

const viewport = { width: 1000, height: 700 }

describe('placeMenu', () => {
  it('opens exactly at the pointer when the menu fits', () => {
    expect(placeMenu(120, 200, 210, 260, viewport)).toEqual({ left: 120, top: 200 })
  })

  it('shifts up only as far as the bottom edge requires', () => {
    // 600 + 260 + margin overflows, so the menu ends at the viewport bottom
    // instead of flipping above the pointer or being cut off.
    expect(placeMenu(120, 600, 210, 260, viewport)).toEqual({
      left: 120,
      top: viewport.height - 260 - 8,
    })
  })

  it('shifts left only as far as the right edge requires', () => {
    expect(placeMenu(900, 200, 210, 120, viewport)).toEqual({
      left: viewport.width - 210 - 8,
      top: 200,
    })
  })

  it('keeps a menu taller than the window at the top margin', () => {
    const short = { width: 800, height: 420 }
    expect(placeMenu(120, 300, 210, 500, short)).toEqual({ left: 120, top: 8 })
  })

  it('never leaves the viewport', () => {
    const tiny = { width: 300, height: 200 }
    const placement = placeMenu(280, 190, 210, 150, tiny)
    expect(placement.left).toBeGreaterThanOrEqual(8)
    expect(placement.left + 210).toBeLessThanOrEqual(tiny.width - 8)
    expect(placement.top).toBeGreaterThanOrEqual(8)
    expect(placement.top + 150).toBeLessThanOrEqual(tiny.height - 8)
  })
})

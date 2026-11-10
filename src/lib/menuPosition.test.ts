import { describe, expect, it } from 'vitest'
import { placeMenu, placeSubmenu } from './menuPosition'

const viewport = { width: 1000, height: 700 }

describe('placeMenu', () => {
  it('opens exactly at the pointer when the menu fits', () => {
    expect(placeMenu(120, 200, 210, 260, viewport)).toEqual({ left: 120, top: 200 })
  })

  it('flips above the pointer when the menu would cross the bottom edge', () => {
    expect(placeMenu(120, 600, 210, 260, viewport)).toEqual({
      left: 120,
      top: 340,
    })
  })

  it('flips to the left of the pointer when the menu would cross the right edge', () => {
    expect(placeMenu(900, 200, 210, 120, viewport)).toEqual({
      left: 690,
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

describe('placeSubmenu', () => {
  it('uses the side with more room and caps the panel to that space', () => {
    expect(
      placeSubmenu(
        { left: 8, right: 210, top: 100, bottom: 130 },
        { width: 420, height: 200 },
        { width: 610, height: 400 },
        'right',
      ),
    ).toEqual({ left: false, up: false, maxWidth: 385 })
  })

  it('flips a submenu up when the lower edge has less room', () => {
    expect(
      placeSubmenu(
        { left: 100, right: 200, top: 740, bottom: 770 },
        { width: 160, height: 300 },
        { width: 800, height: 780 },
        'right',
      ).up,
    ).toBe(true)
  })
})

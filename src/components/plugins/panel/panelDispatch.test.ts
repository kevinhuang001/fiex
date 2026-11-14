import { describe, expect, it } from 'vitest'
import { canDispatchPanelMethod, initialPanelSize, resizedPanelSize } from './panelDispatch'

describe('panel dispatch visibility', () => {
  it('blocks operations while hidden except for the cached-panel completion methods', () => {
    expect(canDispatchPanelMethod('plugin.invoke', false)).toBe(false)
    expect(canDispatchPanelMethod('dialog.openFile', false)).toBe(false)
    expect(canDispatchPanelMethod('context.get', false)).toBe(true)
    expect(canDispatchPanelMethod('tasks.finish', false)).toBe(true)
    expect(canDispatchPanelMethod('panel.resize', false)).toBe(true)
    expect(canDispatchPanelMethod('plugin.invoke', true)).toBe(true)
  })
})

describe('plugin panel sizing', () => {
  it('uses the default size and caps it to the viewport', () => {
    expect(initialPanelSize(undefined, { width: 1200, height: 900 })).toEqual({
      width: 560,
      height: 560,
    })
    expect(initialPanelSize({ width: 600, height: 500 }, { width: 500, height: 400 })).toEqual({
      width: 460,
      height: 308,
    })
  })

  it('clamps requested resize dimensions and rejects incomplete requests', () => {
    expect(resizedPanelSize({ width: 100, height: 900 }, { width: 1200, height: 900 })).toEqual({
      width: 320,
      height: 680,
    })
    expect(resizedPanelSize({ width: 500 }, { width: 1200, height: 900 })).toBeNull()
    expect(
      resizedPanelSize({ width: Number.NaN, height: 300 }, { width: 1200, height: 900 }),
    ).toBeNull()
  })
})

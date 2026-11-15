import { describe, expect, it } from 'vitest'
import { moveVisiblePathSelection, selectAllVisiblePaths, selectVisiblePath } from './fileSelection'

const visiblePaths = ['/a', '/b', '/c', '/d']

describe('selectVisiblePath', () => {
  it('selects one path and moves the range anchor', () => {
    expect(
      selectVisiblePath({
        visiblePaths,
        selectedPaths: ['/a', '/c'],
        clickedPath: '/b',
        anchorPath: '/a',
        extendRange: false,
        additive: false,
      }),
    ).toEqual({ selectedPaths: ['/b'], anchorPath: '/b' })
  })

  it('toggles an additive selection while preserving the other paths', () => {
    expect(
      selectVisiblePath({
        visiblePaths,
        selectedPaths: ['/a', '/c'],
        clickedPath: '/b',
        anchorPath: '/a',
        extendRange: false,
        additive: true,
      }),
    ).toEqual({ selectedPaths: ['/a', '/c', '/b'], anchorPath: '/b' })
    expect(
      selectVisiblePath({
        visiblePaths,
        selectedPaths: ['/a', '/b', '/c'],
        clickedPath: '/b',
        anchorPath: '/a',
        extendRange: false,
        additive: true,
      }).selectedPaths,
    ).toEqual(['/a', '/c'])
  })

  it('selects the inclusive visible range and keeps its original anchor', () => {
    expect(
      selectVisiblePath({
        visiblePaths,
        selectedPaths: ['/d'],
        clickedPath: '/c',
        anchorPath: '/a',
        extendRange: true,
        additive: false,
      }),
    ).toEqual({ selectedPaths: ['/a', '/b', '/c'], anchorPath: '/a' })
  })

  it('starts a new range when the previous anchor is no longer visible', () => {
    expect(
      selectVisiblePath({
        visiblePaths: ['/b', '/c', '/d'],
        selectedPaths: ['/d'],
        clickedPath: '/c',
        anchorPath: '/a',
        extendRange: true,
        additive: false,
      }),
    ).toEqual({ selectedPaths: ['/c'], anchorPath: '/c' })
  })
})

describe('moveVisiblePathSelection', () => {
  const move = (key: string, selectedPaths: string[] = ['/b'], extendSelection = false) =>
    moveVisiblePathSelection({
      visiblePaths,
      selectedPaths,
      key,
      extendSelection,
      disabled: false,
    })

  it('moves one item with arrow keys and clamps at either end', () => {
    expect(move('ArrowDown')).toEqual({ focusedPath: '/c', selectedPaths: ['/c'] })
    expect(move('ArrowLeft', ['/a'])).toEqual({ focusedPath: '/a', selectedPaths: ['/a'] })
    expect(move('ArrowUp', ['/d'])).toEqual({ focusedPath: '/c', selectedPaths: ['/c'] })
  })

  it('starts at the first item when the current selection is outside the view', () => {
    expect(move('ArrowDown', ['/missing'])).toEqual({
      focusedPath: '/a',
      selectedPaths: ['/a'],
    })
  })

  it('jumps to the first and last visible item', () => {
    expect(move('Home')).toEqual({ focusedPath: '/a', selectedPaths: ['/a'] })
    expect(move('End')).toEqual({ focusedPath: '/d', selectedPaths: ['/d'] })
  })

  it('extends selection without duplicating the focused path', () => {
    expect(move('ArrowDown', ['/a', '/b', '/c', '/d'], true)).toEqual({
      focusedPath: '/d',
      selectedPaths: ['/a', '/b', '/c', '/d'],
    })
  })

  it('ignores unsupported keys, disabled movement, and empty views', () => {
    expect(move('Enter')).toBeNull()
    expect(
      moveVisiblePathSelection({
        visiblePaths,
        selectedPaths: ['/b'],
        key: 'ArrowDown',
        extendSelection: false,
        disabled: true,
      }),
    ).toBeNull()
    expect(
      moveVisiblePathSelection({
        visiblePaths: [],
        selectedPaths: [],
        key: 'ArrowDown',
        extendSelection: false,
        disabled: false,
      }),
    ).toBeNull()
  })

  it('selects all visible paths without exposing the input array', () => {
    const selected = selectAllVisiblePaths(visiblePaths)
    expect(selected).toEqual(visiblePaths)
    expect(selected).not.toBe(visiblePaths)
  })
})

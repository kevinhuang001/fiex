import { describe, expect, it } from 'vitest'
import {
  closeNavigationTab,
  moveNavigationTab,
  openNavigationTab,
  selectNavigationTab,
} from './navigationTabs'

const tabs = ['/a', '/b', '/c']
const histories = [
  { paths: ['/a', '/a/child'], cursor: 1 },
  { paths: ['/b'], cursor: 0 },
  { paths: ['/c', '/c/child'], cursor: 0 },
]

describe('moveNavigationTab', () => {
  it('moves the active tab together with its current navigation history', () => {
    const result = moveNavigationTab({
      tabs,
      histories,
      active: 0,
      currentHistory: { paths: ['/a', '/a/current'], cursor: 1 },
      from: 0,
      to: 2,
    })

    expect(result).toEqual({
      tabs: ['/b', '/c', '/a'],
      histories: [
        { paths: ['/b'], cursor: 0 },
        { paths: ['/c', '/c/child'], cursor: 0 },
        { paths: ['/a', '/a/current'], cursor: 1 },
      ],
      active: 2,
    })
    expect(tabs).toEqual(['/a', '/b', '/c'])
    expect(histories[0]).toEqual({ paths: ['/a', '/a/child'], cursor: 1 })
  })

  it('adjusts the active index when another tab crosses it', () => {
    const movedLeft = moveNavigationTab({
      tabs,
      histories,
      active: 1,
      currentHistory: histories[1]!,
      from: 0,
      to: 2,
    })
    const movedRight = moveNavigationTab({
      tabs,
      histories,
      active: 1,
      currentHistory: histories[1]!,
      from: 2,
      to: 0,
    })

    expect(movedLeft?.active).toBe(0)
    expect(movedRight?.active).toBe(2)
  })

  it('leaves unrelated active tabs in place and rejects no-op or invalid moves', () => {
    const input = {
      tabs,
      histories,
      active: 0,
      currentHistory: histories[0]!,
    }
    const moved = moveNavigationTab({
      ...input,
      from: 1,
      to: 2,
    })

    expect(moved?.active).toBe(0)
    expect(moveNavigationTab({ ...input, from: 1, to: 1 })).toBeNull()
    expect(moveNavigationTab({ ...input, from: -1, to: 1 })).toBeNull()
  })
})

describe('navigation tab transitions', () => {
  it('opens a tab after saving the active tab’s latest history', () => {
    const result = openNavigationTab({
      tabs,
      histories,
      active: 1,
      currentHistory: { paths: ['/b', '/b/current'], cursor: 1 },
      path: '/new',
    })

    expect(result).toEqual({
      tabs: ['/a', '/b', '/c', '/new'],
      histories: [
        histories[0],
        { paths: ['/b', '/b/current'], cursor: 1 },
        histories[2],
        { paths: ['/new'], cursor: 0 },
      ],
      active: 3,
      path: '/new',
      history: { paths: ['/new'], cursor: 0 },
    })
    expect(histories[1]).toEqual({ paths: ['/b'], cursor: 0 })
  })

  it('switches to a saved history and persists the active tab’s current history', () => {
    const result = selectNavigationTab({
      tabs,
      histories,
      active: 0,
      currentHistory: { paths: ['/a', '/a/current'], cursor: 1 },
      index: 2,
    })

    expect(result).toEqual({
      tabs,
      histories: [{ paths: ['/a', '/a/current'], cursor: 1 }, histories[1], histories[2]],
      active: 2,
      path: '/c',
      history: histories[2],
    })
    expect(
      selectNavigationTab({
        tabs,
        histories,
        active: 0,
        currentHistory: histories[0]!,
        index: -1,
      }),
    ).toBeNull()
    expect(
      selectNavigationTab({
        tabs,
        histories,
        active: 0,
        currentHistory: histories[0]!,
        index: 0,
      }),
    ).toBeNull()
  })

  it('closes an inactive tab without changing the selected tab history', () => {
    const result = closeNavigationTab({
      tabs,
      histories,
      active: 2,
      currentHistory: { paths: ['/c', '/c/current'], cursor: 1 },
      index: 0,
      startPath: '/recent',
    })

    expect(result).toEqual({
      tabs: ['/b', '/c'],
      histories: [histories[1], { paths: ['/c', '/c/current'], cursor: 1 }],
      active: 1,
      path: '/c',
      history: { paths: ['/c', '/c/current'], cursor: 1 },
    })
  })

  it('closes the active tab onto its neighbor history', () => {
    const result = closeNavigationTab({
      tabs,
      histories,
      active: 1,
      currentHistory: { paths: ['/b', '/b/current'], cursor: 1 },
      index: 1,
      startPath: '/recent',
    })

    expect(result).toEqual({
      tabs: ['/a', '/c'],
      histories: [histories[0], histories[2]],
      active: 1,
      path: '/c',
      history: histories[2],
    })
  })

  it('closes the final tab onto the configured start path and rejects invalid closes', () => {
    const result = closeNavigationTab({
      tabs: ['/last'],
      histories: [{ paths: ['/last', '/last/child'], cursor: 1 }],
      active: 0,
      currentHistory: { paths: ['/last', '/last/child'], cursor: 1 },
      index: 0,
      startPath: '/configured-start',
    })

    expect(result).toEqual({
      tabs: ['/configured-start'],
      histories: [{ paths: ['/configured-start'], cursor: 0 }],
      active: 0,
      path: '/configured-start',
      history: { paths: ['/configured-start'], cursor: 0 },
    })
    expect(
      closeNavigationTab({
        tabs,
        histories,
        active: 0,
        currentHistory: histories[0]!,
        index: -1,
        startPath: '/recent',
      }),
    ).toBeNull()
  })
})

import { describe, expect, it } from 'vitest'
import { moveNavigationTab } from './navigationTabs'

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

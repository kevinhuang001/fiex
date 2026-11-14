import { describe, expect, it } from 'vitest'
import {
  appendNavigationPath,
  moveNavigationCursor,
  previousNavigationPath,
} from './navigationHistory'

describe('navigationHistory', () => {
  it('truncates the forward branch when a new path is visited', () => {
    const history = ['/work', '/work/src', '/work/old']
    const result = appendNavigationPath(history, 1, '/work/tests')

    expect(result).toEqual({ history: ['/work', '/work/src', '/work/tests'], cursor: 2 })
    expect(history).toEqual(['/work', '/work/src', '/work/old'])
  })

  it('starts a new history when the current cursor is before the first entry', () => {
    expect(appendNavigationPath([], -1, '/work')).toEqual({ history: ['/work'], cursor: 0 })
  })

  it('moves back and forward only when a destination exists', () => {
    const history = ['/work', '/work/src', '/work/src/ui']

    expect(moveNavigationCursor(history, 2, -1)).toEqual({ path: '/work/src', cursor: 1 })
    expect(moveNavigationCursor(history, 1, 1)).toEqual({ path: '/work/src/ui', cursor: 2 })
    expect(moveNavigationCursor(history, 0, -1)).toBeNull()
    expect(moveNavigationCursor(history, 2, 1)).toBeNull()
    expect(moveNavigationCursor(history, 3, -1)).toBeNull()
    expect(previousNavigationPath(history, 2)).toBe('/work/src')
    expect(previousNavigationPath([], -1)).toBeNull()
  })
})

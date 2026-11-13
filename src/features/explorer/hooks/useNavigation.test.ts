import { describe, expect, it } from 'vitest'
import { previousNavigationPath } from './useNavigation'

describe('previousNavigationPath', () => {
  it('returns the directory visited immediately before the current one', () => {
    expect(previousNavigationPath(['/work', '/work/src', '/work/src/ui'], 2)).toBe('/work/src')
  })

  it('returns null when there is no earlier directory in the current history', () => {
    expect(previousNavigationPath(['/work'], 0)).toBeNull()
    expect(previousNavigationPath([], -1)).toBeNull()
  })
})

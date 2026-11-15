import { describe, expect, it } from 'vitest'
import { hasTopLevelMenuActions } from './menuVisibility'

describe('context menu visibility', () => {
  it('does not show a menu without an actionable top-level item', () => {
    expect(hasTopLevelMenuActions([])).toBe(false)
    expect(hasTopLevelMenuActions([{ items: [{ kind: 'separator' }] }])).toBe(false)
    expect(hasTopLevelMenuActions([{ items: [{ kind: 'action', parentId: 'parent' }] }])).toBe(
      false,
    )
  })

  it('shows a menu with a top-level action or submenu', () => {
    expect(hasTopLevelMenuActions([{ items: [{ kind: 'action' }] }])).toBe(true)
    expect(hasTopLevelMenuActions([{ items: [{ kind: 'submenu' }] }])).toBe(true)
  })
})

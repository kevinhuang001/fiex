import { describe, expect, it } from 'vitest'
import { canRenameEntry } from './renamePolicy'

describe('canRenameEntry', () => {
  it('requires an explicit rename capability', () => {
    expect(canRenameEntry(undefined)).toBe(false)
    expect(canRenameEntry({})).toBe(false)
    expect(canRenameEntry({ allowedOps: [] })).toBe(false)
    expect(canRenameEntry({ allowedOps: ['read'] })).toBe(false)
  })

  it('allows rename when the provider grants it', () => {
    expect(canRenameEntry({ allowedOps: ['read', 'rename'] })).toBe(true)
  })
})

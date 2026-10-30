import { describe, expect, it } from 'vitest'
import { activeError, reportError, dismissError } from './errorCenter'

describe('error notification queue', () => {
  it('redacts before enqueueing and aggregates duplicates without dropping unrelated errors', () => {
    reportError('Authorization: Bearer HIDDEN', 'test')
    expect(activeError()?.message).not.toContain('HIDDEN')
    const id = activeError()!.id
    reportError('Authorization: Bearer HIDDEN', 'test')
    reportError('unsupported filename failed', 'test')
    dismissError(id)
    expect(activeError()?.message).toBe('unsupported filename failed')
    dismissError(activeError()!.id)
    expect(activeError()).toBeNull()
  })
})

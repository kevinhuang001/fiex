import { describe, expect, it } from 'vitest'
import { activeError, dismissError, isFilesystemPermissionError, reportError } from './errorCenter'

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

it('routes OS permission failures to the filesystem error center', () => {
  for (const message of [
    'Permission denied (os error 13)',
    'Operation not permitted (os error 1)',
    'Read-only file system (os error 30)',
    'Access is denied. (os error 5)',
    'PermissionDenied: USB device is read-only',
  ]) {
    expect(isFilesystemPermissionError(message)).toBe(true)
  }
  expect(isFilesystemPermissionError('Unsupported: provider does not implement write')).toBe(false)
})

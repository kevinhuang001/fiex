import { describe, expect, it } from 'vitest'
import { shouldRestoreAutostartSessions } from './autostartSessions'

describe('shouldRestoreAutostartSessions', () => {
  const ready = {
    native: true,
    alreadyStarted: false,
    hasListing: true,
    connectionsLoaded: true,
  }

  it('waits for both the first listing and the persisted connection registry', () => {
    expect(shouldRestoreAutostartSessions({ ...ready, hasListing: false })).toBe(false)
    expect(shouldRestoreAutostartSessions({ ...ready, connectionsLoaded: false })).toBe(false)
  })

  it('restores native sessions once both startup sources are ready', () => {
    expect(shouldRestoreAutostartSessions(ready)).toBe(true)
    expect(shouldRestoreAutostartSessions({ ...ready, alreadyStarted: true })).toBe(false)
    expect(shouldRestoreAutostartSessions({ ...ready, native: false })).toBe(false)
  })
})

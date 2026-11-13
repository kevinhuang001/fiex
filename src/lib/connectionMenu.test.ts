import { describe, expect, it } from 'vitest'
import { parseConnectionMenuAction } from './connectionMenu'

describe('parseConnectionMenuAction', () => {
  it('decodes the Core-owned edit and close actions', () => {
    expect(
      parseConnectionMenuAction(
        {
          kind: 'coreAction',
          command: 'editConnection',
          connectionId: 'connection-1',
        },
        'connection-1',
      ),
    ).toEqual({
      kind: 'coreAction',
      command: 'editConnection',
      connectionId: 'connection-1',
    })
    expect(
      parseConnectionMenuAction(
        {
          kind: 'coreAction',
          command: 'closeConnection',
          connectionId: 'connection-1',
        },
        'connection-1',
      )?.command,
    ).toBe('closeConnection')
  })

  it('passes non-Core results through and rejects malformed Core actions', () => {
    expect(parseConnectionMenuAction({ handled: true }, 'connection-1')).toBeNull()
    expect(parseConnectionMenuAction({ kind: 'custom', payload: 1 }, 'connection-1')).toBeNull()
    expect(() =>
      parseConnectionMenuAction(
        {
          kind: 'coreAction',
          command: 'openPath',
          connectionId: 'connection-1',
        },
        'connection-1',
      ),
    ).toThrow(TypeError)
    expect(() =>
      parseConnectionMenuAction(
        {
          kind: 'coreAction',
          command: 'closeConnection',
          connectionId: 'connection-1',
          extra: true,
        },
        'connection-1',
      ),
    ).toThrow(TypeError)
    expect(() =>
      parseConnectionMenuAction(
        { kind: 'coreAction', command: 'closeConnection', connectionId: 'other' },
        'connection-1',
      ),
    ).toThrow(TypeError)
  })
})

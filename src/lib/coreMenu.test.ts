import { describe, expect, it } from 'vitest'
import { parseCoreMenuAction } from './coreMenu'

describe('parseCoreMenuAction', () => {
  it('decodes a Core action with a supported command', () => {
    expect(
      parseCoreMenuAction({
        kind: 'coreAction',
        command: 'batchRename',
        paths: ['/a', '/b'],
        path: '/a',
        directory: '/',
      }),
    ).toEqual({
      kind: 'coreAction',
      command: 'batchRename',
      paths: ['/a', '/b'],
      path: '/a',
      directory: '/',
    })
  })

  it('passes non-Core results through for the caller to handle', () => {
    expect(parseCoreMenuAction(null)).toBeNull()
    expect(parseCoreMenuAction({ kind: 'handled', handled: true })).toBeNull()
    expect(parseCoreMenuAction({ kind: 'panel', assetPath: '/plugin/panel.html' })).toBeNull()
  })

  it('rejects unknown commands, malformed fields and unexpected keys', () => {
    const valid = {
      kind: 'coreAction',
      command: 'open',
      paths: ['/a'],
      path: '/a',
      directory: '/',
    }
    expect(() => parseCoreMenuAction({ ...valid, command: 'runShell' })).toThrow(TypeError)
    expect(() => parseCoreMenuAction({ ...valid, paths: [42] })).toThrow(TypeError)
    expect(() => parseCoreMenuAction({ ...valid, extra: true })).toThrow(TypeError)
  })
})

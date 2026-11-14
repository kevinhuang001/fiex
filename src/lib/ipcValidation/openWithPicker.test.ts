import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'

describe('open-with picker IPC result', () => {
  const response = {
    sessionId: 'session-1',
    items: [
      { id: 'app-1', name: 'Text Editor', iconSource: '/Applications/editor.app' },
      { id: 'app-2', name: 'Plain Text', iconSource: null },
    ],
  }

  it('decodes the Rust camelCase response and nullable icon source', () => {
    expect(decodeDynamicIpcResult('open_with_picker_candidates', {}, response)).toEqual(response)
  })

  it.each([
    { ...response, extra: true },
    { ...response, items: [{ id: 'app-1', name: 'Editor' }] },
    { ...response, items: [{ id: 'app-1', name: 'Editor', iconSource: 4 }] },
    { ...response, items: [{ id: 'app-1', name: 'Editor', iconSource: null, extra: true }] },
  ])('rejects malformed or unknown response fields', (value) => {
    expect(() => decodeDynamicIpcResult('open_with_picker_candidates', {}, value)).toThrow(
      'Invalid open_with_picker_candidates IPC result',
    )
  })

  it('rejects commands without a registered runtime decoder', () => {
    expect(() => decodeDynamicIpcResult('not_a_command', {}, null)).toThrow(
      'No IPC result decoder registered for not_a_command',
    )
  })
})

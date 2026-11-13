import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'

describe('plugin_before_open result validation', () => {
  it('accepts the strict HookResult DTO', () => {
    expect(decodeDynamicIpcResult('plugin_before_open', {}, { handled: true })).toEqual({
      handled: true,
    })
  })

  it.each([null, {}, { handled: 'true' }, { handled: false, extra: true }])(
    'rejects malformed hook results',
    (value) => {
      expect(() => decodeDynamicIpcResult('plugin_before_open', {}, value)).toThrow(
        'Invalid before-open result',
      )
    },
  )
})

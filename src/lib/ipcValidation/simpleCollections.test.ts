import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { decodeSimpleCollectionIpcResult } from './simpleCollections'

describe('simple collection result validation', () => {
  it.each([
    ['plugin_files_dialog', ['/home/a.txt', '/home/b.txt']],
    ['plugin_network_domains', ['example.test', 'api.example.test']],
    ['plugin_release_assets', ['plugin.zip']],
    ['register_drag', ['drag-token']],
    ['system_fonts', ['Inter', 'Noto Sans']],
  ])('accepts string arrays from %s', (command, value) => {
    expect(decodeSimpleCollectionIpcResult(command, value)).toEqual({ value })
  })

  it.each([
    ['plugin_files_dialog', ['ok', 1]],
    ['plugin_network_domains', {}],
    ['plugin_release_assets', null],
    ['register_drag', 'token'],
    ['system_fonts', [null]],
  ])('rejects malformed string arrays from %s', (command, value) => {
    expect(() => decodeSimpleCollectionIpcResult(command, value)).toThrow(
      `Invalid ${command} IPC result`,
    )
  })

  it('constructs plugin status tuples and rejects malformed records', () => {
    expect(decodeSimpleCollectionIpcResult('plugin_status', [['archive', true]])).toEqual({
      value: [['archive', true]],
    })
    for (const value of [[['archive']], [['archive', 'yes']], [['archive', false, 'extra']], {}]) {
      expect(() => decodeSimpleCollectionIpcResult('plugin_status', value)).toThrow(
        'Invalid plugin_status IPC result',
      )
    }
  })

  it('leaves other collection results to their dedicated decoders', () => {
    expect(decodeSimpleCollectionIpcResult('detect_ides', [])).toBeNull()
  })

  it('validates collection results at the dynamic dispatch boundary', () => {
    expect(decodeDynamicIpcResult('system_fonts', {}, ['Inter'])).toEqual(['Inter'])
    expect(() => decodeDynamicIpcResult('system_fonts', {}, [1])).toThrow(
      'Invalid system_fonts IPC result',
    )
    expect(decodeDynamicIpcResult('plugin_status', {}, [['archive', true]])).toEqual([
      ['archive', true],
    ])
  })
})

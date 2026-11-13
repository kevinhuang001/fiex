import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { decodeSimpleIpcResult } from './simpleResults'

describe('simple Tauri result validation', () => {
  it.each([
    ['create_entry', '/home/report.txt'],
    ['credential_backend', 'keychain'],
    ['rename_entry', 'report-renamed.txt'],
  ])('accepts string results from %s', (command, value) => {
    expect(decodeSimpleIpcResult(command, value)).toEqual({ value })
  })

  it.each([
    ['pick_folder', null],
    ['initial_path', '/home'],
    ['trash_path', null],
  ])('accepts nullable string results from %s', (command, value) => {
    expect(decodeSimpleIpcResult(command, value)).toEqual({ value })
  })

  it.each([
    ['delete_permanently', 0],
    ['empty_trash', 12],
    ['restore_from_trash', 3],
  ])('accepts unsigned counts from %s', (command, value) => {
    expect(decodeSimpleIpcResult(command, value)).toEqual({ value })
  })

  it.each([
    ['delete_permanently', -1],
    ['empty_trash', 1.5],
    ['restore_from_trash', Number.MAX_SAFE_INTEGER + 1],
  ])('rejects invalid counts from %s', (command, value) => {
    expect(() => decodeSimpleIpcResult(command, value)).toThrow(`Invalid ${command} IPC result`)
  })

  it('accepts only nullish unit results and booleans', () => {
    expect(decodeSimpleIpcResult('open_path', null)).toEqual({ value: undefined })
    expect(decodeSimpleIpcResult('open_terminal', undefined)).toEqual({ value: undefined })
    expect(decodeSimpleIpcResult('remote_cached', true)).toEqual({ value: true })
    expect(() => decodeSimpleIpcResult('open_path', false)).toThrow('Invalid open_path IPC result')
    expect(() => decodeSimpleIpcResult('remote_cached', 'true')).toThrow(
      'Invalid remote_cached IPC result',
    )
  })

  it('requires toolbar invocations to return the empty Rust DTO', () => {
    expect(decodeSimpleIpcResult('plugin_toolbar_invoke', {})).toEqual({ value: undefined })
    expect(decodeDynamicIpcResult('plugin_toolbar_invoke', {}, {})).toBeUndefined()
    expect(() => decodeDynamicIpcResult('plugin_toolbar_invoke', {}, { result: true })).toThrow(
      'Invalid plugin_toolbar_invoke IPC result',
    )
    expect(() => decodeDynamicIpcResult('plugin_toolbar_invoke', {}, null)).toThrow(
      'Invalid plugin_toolbar_invoke IPC result',
    )
  })

  it.each(['macos', 'windows', 'linux'])('accepts the Rust %s platform result', (platform) => {
    expect(decodeSimpleIpcResult('platform', platform)).toEqual({ value: platform })
    expect(decodeDynamicIpcResult('platform', {}, platform)).toBe(platform)
  })

  it.each(['', 'darwin', 'win32', 'freebsd', null])(
    'rejects unsupported platform result %s',
    (value) => {
      expect(() => decodeSimpleIpcResult('platform', value)).toThrow('Invalid platform IPC result')
    },
  )

  it('leaves commands with structured result decoders unhandled', () => {
    expect(decodeSimpleIpcResult('list_directory', { entries: [] })).toBeNull()
  })

  it('validates simple results at the dynamic dispatch boundary', () => {
    expect(() => decodeDynamicIpcResult('create_entry', {}, 12)).toThrow(
      'Invalid create_entry IPC result',
    )
    expect(decodeDynamicIpcResult('create_entry', {}, '/home/report.txt')).toBe('/home/report.txt')
    expect(() => decodeDynamicIpcResult('open_path', {}, { unexpected: true })).toThrow(
      'Invalid open_path IPC result',
    )
  })
})

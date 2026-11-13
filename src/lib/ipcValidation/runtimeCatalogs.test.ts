import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { decodeRuntimeCatalogIpcResult } from './runtimeCatalogs'

describe('runtime catalog result validation', () => {
  it('constructs IDE and terminal records from the wire payload', () => {
    expect(
      decodeRuntimeCatalogIpcResult('detect_ides', [{ id: 'vscode', name: 'VS Code' }]),
    ).toEqual({ value: [{ id: 'vscode', name: 'VS Code' }] })
    expect(
      decodeRuntimeCatalogIpcResult('detect_terminals', [{ id: 'kitty', name: 'kitty' }]),
    ).toEqual({ value: [{ id: 'kitty', name: 'kitty' }] })
  })

  it('requires all shell choice fields', () => {
    expect(
      decodeRuntimeCatalogIpcResult('detect_shells', [
        { id: 'zsh', name: 'Zsh', path: '/bin/zsh' },
      ]),
    ).toEqual({ value: [{ id: 'zsh', name: 'Zsh', path: '/bin/zsh' }] })
    expect(() =>
      decodeRuntimeCatalogIpcResult('detect_shells', [{ id: 'zsh', name: 'Zsh' }]),
    ).toThrow('Invalid detect_shells IPC result')
  })

  it.each([
    ['detect_ides', [{ id: 'vscode', name: 'VS Code', extra: true }]],
    ['detect_terminals', [{ id: '', name: 'Terminal' }]],
    ['detect_terminals', { id: 'kitty', name: 'kitty' }],
    ['detect_shells', [{ id: 'zsh', name: 'Zsh', path: '' }]],
  ])('rejects malformed %s result items', (command, value) => {
    expect(() => decodeRuntimeCatalogIpcResult(command, value)).toThrow(
      `Invalid ${command} IPC result`,
    )
  })

  it('validates runtime catalogs at the dynamic result boundary', () => {
    expect(decodeDynamicIpcResult('detect_ides', {}, [{ id: 'zed', name: 'Zed' }])).toEqual([
      { id: 'zed', name: 'Zed' },
    ])
    expect(() =>
      decodeDynamicIpcResult('detect_shells', {}, [{ id: 'bash', name: 'Bash' }]),
    ).toThrow('Invalid detect_shells IPC result')
  })

  it('leaves other structured commands for their own decoder', () => {
    expect(decodeRuntimeCatalogIpcResult('remote_connections', [])).toBeNull()
  })
})

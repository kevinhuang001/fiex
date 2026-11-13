import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { parsePluginCatalog } from './pluginCatalog'

const plugin = {
  schemaVersion: 2,
  id: 'org.example.viewer',
  name: 'Example Viewer',
  version: '1.2.3',
  description: 'Example plugin',
  icon: null,
  extensions: { toolbar: { title: 'Open' } },
  uiTimeoutMs: 15_000,
  backendAvailable: true,
  enabled: true,
  author: { name: 'Example Org', url: null },
  approved: true,
  declaredPermissions: ['files.read'],
  grantedPermissions: ['files.read'],
  requestedNetworkAll: false,
  requestedNetworkDomains: [],
  allowedNetworkAll: false,
  allowedNetworkDomains: [],
  requestedCommands: ['viewer-cli'],
  allowedCommands: ['viewer-cli'],
  missingCommands: [],
}

describe('parsePluginCatalog', () => {
  it('decodes installed plugin DTOs and normalizes nullable optional fields', () => {
    const decoded = parsePluginCatalog([plugin])
    expect(decoded[0]).not.toHaveProperty('icon')
    expect(decoded[0]).toMatchObject({
      schemaVersion: 2,
      id: 'org.example.viewer',
      author: { name: 'Example Org', url: null },
      extensions: { toolbar: { title: 'Open' } },
    })
    expect(decodeDynamicIpcResult('plugins_list', {}, [plugin])).toEqual(decoded)
  })

  it('accepts null author, icon and extensions from Rust Option/Value fields', () => {
    const decoded = parsePluginCatalog([
      { ...plugin, icon: null, author: null, extensions: null },
    ])[0]
    expect(decoded).not.toHaveProperty('icon')
    expect(decoded).not.toHaveProperty('author')
    expect(decoded).not.toHaveProperty('extensions')
  })

  it.each([
    { ...plugin, schemaVersion: 3 },
    { ...plugin, enabled: 'true' },
    { ...plugin, requestedNetworkDomains: ['example.com', 3] },
    { ...plugin, missingCommands: null },
    { ...plugin, uiTimeoutMs: 0 },
    { ...plugin, extensions: [] },
    { ...plugin, author: { name: 'Example Org', url: 42 } },
    { ...plugin, unknown: true },
  ])('rejects malformed installed plugin records', (invalidPlugin) => {
    expect(() => parsePluginCatalog([invalidPlugin])).toThrow('Invalid plugins_list IPC result')
  })

  it('requires every Rust catalog field and an array result', () => {
    const { author: _author, ...withoutAuthor } = plugin
    expect(() => parsePluginCatalog([withoutAuthor])).toThrow('Invalid plugins_list IPC result')
    expect(() => parsePluginCatalog({ plugins: [plugin] })).toThrow(
      'Invalid plugins_list IPC result',
    )
  })
})

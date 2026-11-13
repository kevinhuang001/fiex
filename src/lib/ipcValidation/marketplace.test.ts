import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { parseMarketplacePlugins } from './marketplace'

const plugin = {
  id: 'org.example.viewer',
  name: 'Example Viewer',
  version: '1.2.3',
  description: '',
  author: { name: 'Example Org', url: null },
  permissions: ['files.read'],
  requestedNetworkDomains: [],
  commands: ['viewer-cli'],
  extensions: { preview: { match: { extensions: ['.example'] } } },
  asset: 'viewer-linux-x86_64.fiexplugin',
  repository: 'kevinhuang001/fiex-official-plugins',
}

describe('parseMarketplacePlugins', () => {
  it('constructs catalog DTOs and preserves JSON extensions', () => {
    expect(parseMarketplacePlugins([plugin])).toEqual([plugin])
    expect(decodeDynamicIpcResult('plugin_marketplace', {}, [plugin])).toEqual([plugin])
  })

  it('accepts absent authors and nullable or arbitrary JSON extensions', () => {
    const { author: _author, ...withoutAuthor } = plugin
    expect(
      parseMarketplacePlugins([
        { ...withoutAuthor, author: null, extensions: null },
        { ...plugin, extensions: ['legacy', 3, false] },
      ]),
    ).toHaveLength(2)
  })

  it.each([
    null,
    {},
    { ...plugin, icon: 'cloud' },
    { ...plugin, permissions: ['files.read', 3] },
    { ...plugin, requestedNetworkDomains: 'example.com' },
    { ...plugin, commands: null },
    { ...plugin, author: { name: 'Example Org', extra: true } },
    { ...plugin, author: { name: '', url: null } },
    { ...plugin, extensions: undefined },
  ])('rejects malformed marketplace entries', (entry) => {
    expect(() => parseMarketplacePlugins([entry])).toThrow('Invalid plugin_marketplace IPC result')
  })

  it('rejects a non-array catalog result', () => {
    expect(() => parseMarketplacePlugins({ plugins: [plugin] })).toThrow(
      'Invalid plugin_marketplace IPC result',
    )
  })
})

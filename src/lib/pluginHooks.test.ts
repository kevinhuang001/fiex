import { describe, expect, it } from 'vitest'
import { beforeOpenHooks } from './pluginHooks'
import type { PluginInfo } from './plugins'
import type { Entry } from './types'

const entry = {
  name: 'b.7z',
  path: '/a/b.7z',
  extension: '7z',
  isDir: false,
} as Entry

const plugin = (id: string, priority: number, hooks = true): PluginInfo => ({
  schemaVersion: 1,
  id,
  name: id,
  version: '1.0.0',
  description: '',
  enabled: true,
  approved: true,
  backendAvailable: true,
  capabilities: [],
  declaredPermissions: hooks ? ['hooks'] : [],
  grantedPermissions: hooks ? ['hooks'] : [],
  requestedCommands: [],
  allowedCommands: [],
  missingCommands: [],
  requestedNetworkDomains: [],
  allowedNetworkDomains: [],
  contributions: {
    hooks: [{ id: 'beforeOpen', event: 'beforeOpen', method: `${id}.open`, priority }],
  },
})

describe('beforeOpenHooks', () => {
  it('orders by priority and breaks ties by id', () => {
    const hooks = beforeOpenHooks([plugin('b', 100), plugin('a', 100), plugin('c', 300)], entry)
    expect(hooks.map((hook) => hook.pluginId)).toEqual(['c', 'a', 'b'])
  })

  it('skips plugins without the hooks permission or approval', () => {
    const disabled = { ...plugin('d', 500), enabled: false }
    const unapproved = { ...plugin('e', 500), approved: false }
    const hooks = beforeOpenHooks(
      [plugin('a', 100), plugin('f', 100, false), disabled, unapproved],
      entry,
    )
    expect(hooks.map((hook) => hook.pluginId)).toEqual(['a'])
  })

  it('filters by extension', () => {
    const zipOnly = plugin('zip', 100)
    zipOnly.contributions!.hooks![0].extensions = ['zip']
    expect(beforeOpenHooks([zipOnly], entry)).toHaveLength(0)
    zipOnly.contributions!.hooks![0].extensions = ['7z']
    expect(beforeOpenHooks([zipOnly], entry)).toHaveLength(1)
  })
})

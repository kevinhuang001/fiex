import { describe, expect, it } from 'vitest'
import { activationGroups } from './pluginPermissions'
import type { PluginInfo } from './plugins'

function plugin(declaredPermissions: string[]): PluginInfo {
  return {
    schemaVersion: 2,
    id: 'org.example.test',
    name: '测试插件',
    version: '1.0.0',
    enabled: true,
    approved: true,
    declaredPermissions,
    grantedPermissions: declaredPermissions,
    requestedNetworkAll: false,
    requestedNetworkDomains: [],
    allowedNetworkAll: false,
    allowedNetworkDomains: [],
    requestedCommands: [],
    allowedCommands: [],
    missingCommands: [],
    backendAvailable: true,
    uiTimeoutMs: 0,
  }
}

describe('activationGroups', () => {
  it('reviews file and folder capabilities in separate groups', () => {
    const groups = activationGroups(
      plugin(['metadata', 'files.read', 'files.write', 'folders.read', 'folders.create']),
    )
    expect(groups.map((group) => group.id)).toEqual(['files', 'folders'])
    expect(groups.map((group) => group.title)).toEqual(['访问文件', '访问文件夹'])
    expect(groups[0].permissions.map((permission) => permission.id)).toEqual([
      'metadata',
      'files.read',
      'files.write',
    ])
    expect(groups[1].permissions.map((permission) => permission.id)).toEqual([
      'folders.read',
      'folders.create',
    ])
  })

  it('keeps network domains and commands with their own capabilities', () => {
    const groups = activationGroups(plugin(['files.read', 'network', 'commands']))
    expect(groups.map((group) => group.id)).toEqual(['files', 'network', 'commands'])
  })
})

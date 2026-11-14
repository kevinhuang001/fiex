import { describe, expect, it } from 'vitest'
import type { RemoteConnection } from '@/lib/types'
import { isOpenConnection, isPluginViewConnection } from './connectionModel'

function connection(mountPath: string): RemoteConnection {
  return {
    id: 'remote-1',
    name: 'Remote',
    kind: 'sftp',
    pluginId: 'ssh',
    mountPath,
    saved: true,
    autostart: false,
    canRead: true,
    canWrite: true,
  }
}

describe('connection sidebar model', () => {
  it('identifies terminal and plugin view connections by their URI scheme', () => {
    expect(isPluginViewConnection(connection('fiex-plugin-view://remote-1'))).toBe(true)
    expect(isPluginViewConnection(connection('fiex://mount/remote-1'))).toBe(false)
    expect(
      isOpenConnection(connection('fiex-plugin-view://remote-1'), ['fiex-plugin-view://remote-1']),
    ).toBe(true)
  })

  it('treats the mount and descendants as open tabs', () => {
    expect(isOpenConnection(connection('/mnt/work'), ['/mnt/work'])).toBe(true)
    expect(isOpenConnection(connection('/mnt/work/'), ['/mnt/work/project'])).toBe(true)
  })

  it('does not match a sibling mount that only shares a string prefix', () => {
    expect(isOpenConnection(connection('/mnt/work'), ['/mnt/workspace'])).toBe(false)
  })
})

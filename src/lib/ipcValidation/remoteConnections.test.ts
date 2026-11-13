import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { decodeRemoteConnectionIpcResult } from './remoteConnections'

const connection = {
  id: 'account-1',
  name: 'Work files',
  kind: 'sftp',
  pluginId: 'org.example.sftp',
  mountPath: 'fiex://connections/account-1',
  saved: true,
  autostart: false,
  canRead: true,
  canWrite: false,
}

describe('remote connection result validation', () => {
  it('constructs connection DTOs for list, save, and session update results', () => {
    expect(decodeRemoteConnectionIpcResult('remote_connections', [connection])).toEqual({
      value: [connection],
    })
    expect(decodeRemoteConnectionIpcResult('save_remote_connection', connection)).toEqual({
      value: connection,
    })
    expect(decodeRemoteConnectionIpcResult('update_connection_session', connection)).toEqual({
      value: connection,
    })
  })

  it.each([
    { ...connection, canWrite: 'false' },
    { ...connection, saved: undefined },
    { ...connection, pluginId: null },
    { ...connection, unexpected: true },
  ])('rejects malformed connection DTOs', (value) => {
    expect(() => decodeRemoteConnectionIpcResult('save_remote_connection', value)).toThrow(
      'Invalid save_remote_connection IPC result',
    )
  })

  it('validates every connection in a returned list', () => {
    expect(() =>
      decodeRemoteConnectionIpcResult('remote_connections', [connection, { ...connection, id: 7 }]),
    ).toThrow('Invalid remote_connections IPC result')
    expect(() => decodeRemoteConnectionIpcResult('remote_connections', connection)).toThrow(
      'Invalid remote_connections IPC result',
    )
  })

  it('validates connection DTOs at the dynamic result boundary', () => {
    expect(decodeDynamicIpcResult('update_connection_session', {}, connection)).toEqual(connection)
    expect(() =>
      decodeDynamicIpcResult('update_connection_session', {}, { ...connection, canRead: 1 }),
    ).toThrow('Invalid update_connection_session IPC result')
  })
})

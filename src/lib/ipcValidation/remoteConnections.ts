import type { RemoteConnection } from '../types'
import { isJsonObject, onlyKeys } from './json'

const CONNECTION_KEYS = [
  'id',
  'name',
  'kind',
  'pluginId',
  'mountPath',
  'saved',
  'autostart',
  'canRead',
  'canWrite',
]

function parseConnection(value: unknown, command: string): RemoteConnection {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, CONNECTION_KEYS) ||
    typeof value.id !== 'string' ||
    !value.id ||
    typeof value.name !== 'string' ||
    !value.name ||
    typeof value.kind !== 'string' ||
    !value.kind ||
    typeof value.pluginId !== 'string' ||
    !value.pluginId ||
    typeof value.mountPath !== 'string' ||
    !value.mountPath ||
    typeof value.saved !== 'boolean' ||
    typeof value.autostart !== 'boolean' ||
    typeof value.canRead !== 'boolean' ||
    typeof value.canWrite !== 'boolean'
  )
    throw new TypeError(`Invalid ${command} IPC result`)

  return {
    id: value.id,
    name: value.name,
    kind: value.kind,
    pluginId: value.pluginId,
    mountPath: value.mountPath,
    saved: value.saved,
    autostart: value.autostart,
    canRead: value.canRead,
    canWrite: value.canWrite,
  }
}

export function decodeRemoteConnectionIpcResult(
  command: string,
  value: unknown,
): { value: unknown } | null {
  if (command === 'remote_connections') {
    if (!Array.isArray(value)) throw new TypeError('Invalid remote_connections IPC result')
    return { value: value.map((item) => parseConnection(item, command)) }
  }
  if (command === 'save_remote_connection' || command === 'update_connection_session')
    return { value: parseConnection(value, command) }
  return null
}

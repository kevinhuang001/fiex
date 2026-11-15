import { call } from '@/lib/api'

/** Core IPC adapter for the app-wide connection list. */
export const connectionServices = {
  remoteConnections: () => call('remote_connections'),
}

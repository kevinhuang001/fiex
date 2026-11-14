import type { RemoteConnection } from '@/lib/types'

export const isPluginViewConnection = (connection: RemoteConnection) =>
  connection.mountPath.startsWith('fiex-plugin-view://')

/** A connection is open when a folder tab targets its mount, or its plugin
 * view is the tab target. Keep open sessions out of the add-session menu. */
export const isOpenConnection = (connection: RemoteConnection, tabs: string[]) => {
  const mount = connection.mountPath.replace(/\/$/, '')
  return tabs.some(
    (path) =>
      path === `fiex-plugin-view://${connection.id}` ||
      (mount && (path === mount || path.startsWith(mount + '/'))),
  )
}

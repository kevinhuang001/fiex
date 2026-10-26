/**
 * Browser-demo permission broker. Native builds never use this: Core blocks
 * the real host call and reports the decision through Tauri events. The demo
 * mirrors that contract so a plugin action can wait for the same Core-owned
 * prompt, which keeps the UI fixture honest.
 */
import type { DemoPlugin } from './plugins/types'

export interface DemoPermissionRequest {
  pluginId: string
  pluginName: string
  icon?: string
  permission: string
  kind: 'permission' | 'network' | 'command'
  target?: string
}

interface Pending {
  request: DemoPermissionRequest
  resolve: (allow: boolean) => void
}

const pending = new Map<string, Pending>()

export function requestDemoPermission(request: DemoPermissionRequest): Promise<boolean> {
  const id = `demo-permission-${crypto.randomUUID()}`
  return new Promise((resolve) => {
    pending.set(id, { request, resolve })
    window.dispatchEvent(
      new CustomEvent('fiex:demo-permission-request', {
        detail: {
          id,
          pluginId: request.pluginId,
          pluginName: request.pluginName,
          icon: request.icon,
          permission: request.permission,
          kind: request.kind,
          target: request.target,
        },
      }),
    )
  })
}

export function resolveDemoPermission(
  id: string,
  allow: boolean,
): DemoPermissionRequest | undefined {
  const entry = pending.get(id)
  if (!entry) return undefined
  pending.delete(id)
  entry.resolve(allow)
  return entry.request
}

/** Apply an allow decision to the demo catalog. Denials only resolve the
 *  pending action; they do not grant anything. */
export function applyDemoPermissionResponse(id: string, allow: boolean, plugins: DemoPlugin[]) {
  const request = resolveDemoPermission(id, allow)
  if (!request || !allow) return
  const plugin = plugins.find((entry) => entry.id === request.pluginId)
  if (!plugin) return
  plugin.grantedPermissions = [...new Set([...plugin.grantedPermissions, request.permission])]
  if (request.kind === 'network' && request.target) {
    plugin.allowedNetworkDomains = [
      ...new Set([...(plugin.allowedNetworkDomains || []), request.target]),
    ]
  }
  if (request.kind === 'command' && request.target) {
    plugin.allowedCommands = [...new Set([...(plugin.allowedCommands || []), request.target])]
  }
}

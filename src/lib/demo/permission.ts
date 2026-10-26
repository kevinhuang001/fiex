/**
 * Browser-demo permission broker. Native builds never use this: Core blocks
 * the real host call and reports the decision through Tauri events. The demo
 * mirrors that contract so a plugin action can wait for the same Core-owned
 * prompt, which keeps the UI fixture honest.
 */
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

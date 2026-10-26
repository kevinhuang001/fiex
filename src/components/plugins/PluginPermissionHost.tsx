import { listen } from '@tauri-apps/api/event'
import { useEffect, useState } from 'react'
import { call, demo, native } from '@/lib/api'
import { refreshPluginCatalog } from '@/lib/plugins'
import {
  PluginPermissionPrompt,
  promptForPermission,
  type PluginPermissionItem,
} from '@/components/settings/PluginPermissionPrompt'

/** A Core-owned request. Plugins cannot construct this event or render the
 * dialog; they only issue a host call and wait for its result. */
interface RuntimePermissionRequest {
  id: string
  pluginId: string
  pluginName: string
  icon?: string | null
  permission: string
  kind: 'permission' | 'network' | 'command'
  target?: string | null
}

function itemFor(request: RuntimePermissionRequest): PluginPermissionItem {
  if (request.kind === 'network') {
    return {
      id: `network:${request.target}`,
      kind: 'network',
      title: '网络域名',
      value: request.target || '',
    }
  }
  if (request.kind === 'command') {
    return {
      id: `command:${request.target}`,
      kind: 'command',
      title: '外部命令',
      value: request.target || '',
    }
  }
  return promptForPermission(request.permission)
}

/**
 * Global host for operation-time permission requests. Core creates one request
 * per plugin/capability/target and blocks the waiting host calls; this
 * component mirrors the queue and reports the user's decision back to Core.
 * The allow decision persists in Core, so the same capability is not asked
 * again until the user switches it off in plugin settings.
 */
export function PluginPermissionHost() {
  const [requests, setRequests] = useState<RuntimePermissionRequest[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  useEffect(() => {
    const receive = (request: RuntimePermissionRequest) => {
      if (!request?.id) return
      setRequests((current) =>
        current.some((entry) => entry.id === request.id) ? current : [...current, request],
      )
    }
    // Browser demo only: lets the local fixture exercise the real Core prompt
    // component without a Tauri event bridge. Native builds never listen here.
    const onDemoRequest = (event: Event) => {
      receive((event as CustomEvent<RuntimePermissionRequest>).detail)
    }
    if (demo) window.addEventListener('fiex:demo-permission-request', onDemoRequest)
    if (!native) {
      return () => window.removeEventListener('fiex:demo-permission-request', onDemoRequest)
    }
    const requested = listen<RuntimePermissionRequest>('plugin-permission-request', ({ payload }) =>
      receive(payload),
    )
    const resolved = listen<{ id?: string }>('plugin-permission-resolved', ({ payload }) => {
      if (!payload?.id) return
      setRequests((current) => current.filter((request) => request.id !== payload.id))
    })
    return () => {
      window.removeEventListener('fiex:demo-permission-request', onDemoRequest)
      void requested.then((stop) => stop())
      void resolved.then((stop) => stop())
    }
  }, [])
  const current = requests[0]
  if (!current) return null
  const respond = async (allow: boolean) => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      await call('plugin_permission_respond', { requestId: current.id, allow })
      setRequests((queue) => queue.filter((request) => request.id !== current.id))
      refreshPluginCatalog()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }
  return (
    <PluginPermissionPrompt
      open
      pluginName={current.pluginName}
      icon={current.icon || undefined}
      item={itemFor(current)}
      step={1}
      total={1}
      busy={busy}
      error={error}
      note={
        current.kind === 'network'
          ? '允许后此精确主机会加入允许列表，直到你在插件设置中移除。拒绝只影响本次操作。'
          : current.kind === 'command'
            ? '允许后此命令会加入允许列表，直到你在插件设置中移除。拒绝只影响本次操作。'
            : '允许后此权限会保持开启，直到你在插件设置中关闭。拒绝只影响本次操作。'
      }
      allowLabel="允许"
      denyLabel="不允许"
      onAllow={() => void respond(true)}
      onDeny={() => void respond(false)}
    />
  )
}

import type { Explorer } from '@/hooks/useExplorer'
import { call } from '@/lib/api'
import type { DialogState } from '@/lib/dialogs'
import { basename } from '@/lib/files'
import type { Dispatch, SetStateAction } from 'react'
import type { Notify, Run } from './types'

/** Volume and remote-connection actions, plus the display names that depend on
 *  them (mounted volumes, remote mounts and the Trash have friendly labels). */
export function useConnectionActions({
  x,
  left,
  right,
  run,
  notify,
  setDialog,
  trashPath,
}: {
  x: Explorer
  left: Explorer
  right: Explorer
  run: Run
  notify: Notify
  setDialog: Dispatch<SetStateAction<DialogState | null>>
  trashPath: string | null
}) {
  const leaveDevice = (path: string) => {
    const home = left.locations.find((location) => location.kind === 'favorite')?.path || '/'
    for (const pane of [left, right]) {
      if (pane.path === path || pane.path.startsWith(path.replace(/[/\\]$/, '') + '/'))
        pane.navigate(home)
    }
  }
  const ejectDevice = (path: string) =>
    setDialog({
      title: '弹出设备',
      submit: '弹出',
      danger: true,
      action: async () => {
        await call('eject_device', { path })
        leaveDevice(path)
        await left.refreshLocations()
      },
    })
  const mountDevice = (device: string) => {
    void run('正在请求挂载…', async () => {
      const path = await call<string>('mount_device', { device })
      await left.refreshLocations()
      x.navigate(path)
      notify('设备已挂载')
    })
  }
  const ejectUsb = (device: string) =>
    setDialog({
      title: '弹出 U 盘',
      submit: '弹出',
      danger: true,
      action: async () => {
        const volume = left.locations.find(
          (location) => location.usbfs && location.device === device,
        )
        await call('eject_usb', { device })
        await left.refreshLocations()
        if (volume) leaveDevice(volume.path)
      },
    })
  const removeConnection = (id: string) => {
    const connection = x.connections.find((item) => item.id === id)
    if (!connection) return
    setDialog({
      title: `关闭“${connection.name}”？`,
      submit: '关闭连接',
      danger: true,
      action: async () => {
        await call('remove_remote_connection', { id })
        await x.refreshConnections()
        if (x.path === connection.mountPath || x.path.startsWith(connection.mountPath + '/')) {
          const home = x.locations.find((location) => location.kind === 'favorite')?.path
          x.navigate(home || '/')
        }
      },
    })
  }
  const remoteConnection = (path: string) =>
    x.connections.find(
      (connection) => path === connection.mountPath || path.startsWith(connection.mountPath + '/'),
    )
  const isRemotePath = (path: string) => Boolean(remoteConnection(path))
  const displayName = (path: string) => {
    if (/^history:\/\/[^/]+\/[^/]+\/?$/.test(path))
      return '历史版本 · ' + path.split('/')[3].slice(0, 8)
    if (path === 'recent://') return '最近使用'
    if (trashPath && path === trashPath) return '废纸篓'
    const volume = left.locations.find((location) => location.usbfs && location.path === path)
    if (volume) return volume.name
    const connection = remoteConnection(path)
    if (connection && path === connection.mountPath) return connection.name
    return basename(path)
  }
  return {
    ejectDevice,
    ejectUsb,
    mountDevice,
    removeConnection,
    remoteConnection,
    isRemotePath,
    displayName,
  }
}

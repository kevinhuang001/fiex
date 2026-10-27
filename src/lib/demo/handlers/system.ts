import type { RemoteConnection } from '@/lib/types'
import { root } from '../data'
import { notHandled } from './shared'

const demoConnections: RemoteConnection[] = [
  {
    id: 'demo-webdav',
    name: '团队 WebDAV',
    kind: 'webdav',
    pluginId: 'org.example.webdav',
    mountPath: 'fiex://mount/demo-webdav/',
    saved: true,
    autostart: false,
  },
  {
    id: 'demo-ssh',
    name: '生产服务器',
    kind: 'ssh',
    pluginId: '__builtin__',
    mountPath: 'fiex-terminal://demo-ssh',
    saved: true,
    autostart: false,
  },
]

const demoConnectionSettings: Record<string, Record<string, string | number | boolean>> = {
  'demo-ssh': {
    host: 'server.example.com',
    port: 22,
    username: 'root',
    terminalType: 'xterm-256color',
  },
  'demo-webdav': { url: 'https://example.com' },
}

let demoUsbMounted = true

export function handleSystem(command: string, a: Record<string, unknown> = {}): unknown {
  switch (command) {
    case 'platform': {
      // `?platform=windows|linux` lets the browser fixture preview the
      // frameless window chrome without rebuilding the desktop app.
      const requested = new URLSearchParams(location.search).get('platform')
      return requested === 'windows' || requested === 'linux' ? requested : 'macos'
    }
    case 'locations':
      return [
        {
          name: '个人文件夹',
          path: root,
          kind: 'favorite',
          available: 0,
          total: 0,
          removable: false,
          filesystem: '',
          device: '',
          mounted: true,
          usbfs: false,
        },
        ...['Desktop', 'Downloads', 'Documents', 'Pictures', 'Music', 'Movies'].map((name, i) => ({
          name: ['桌面', '下载', '文稿', '图片', '音乐', '视频'][i],
          path: root + '/' + name,
          kind: 'favorite',
          available: 0,
          total: 0,
          removable: false,
          filesystem: '',
          device: '',
          mounted: true,
          usbfs: false,
        })),
        {
          name: '应用程序',
          path: root + '/Applications',
          kind: 'applications',
          available: 0,
          total: 0,
          removable: false,
          filesystem: '',
          device: '',
          mounted: true,
          usbfs: false,
        },
        {
          name: 'Macintosh HD',
          path: '/',
          kind: 'disk',
          available: 240e9,
          total: 500e9,
          removable: false,
          filesystem: 'APFS',
          device: '',
          mounted: true,
          usbfs: false,
        },
        ...(demoUsbMounted
          ? [
              {
                name: 'SanDisk USB',
                path: '/Volumes/SanDisk',
                kind: 'disk',
                available: 14.2e9,
                total: 32e9,
                removable: true,
                filesystem: 'exFAT',
                device: '/dev/disk4s1',
                mounted: true,
                usbfs: false,
              },
            ]
          : []),
      ]
    case 'eject_device':
    case 'eject_usb':
      demoUsbMounted = false
      return null
    case 'detect_shells':
      return [
        { id: '/bin/zsh', name: 'zsh', path: '/bin/zsh' },
        { id: '/bin/bash', name: 'bash', path: '/bin/bash' },
        { id: '/bin/sh', name: 'sh', path: '/bin/sh' },
      ]
    case 'detect_terminals':
      return [
        { id: 'default', name: '系统默认终端' },
        { id: 'Terminal', name: 'Terminal' },
        { id: 'iTerm', name: 'iTerm2' },
        { id: 'Tabby', name: 'Tabby' },
      ]
    case 'native_icon':
      throw new Error('UI fixture')
    case 'trash_path':
      return root + '/.Trash'
    case 'open_applications':
      return null
    case 'detect_ides':
      return [
        { id: 'vscode', name: 'VS Code' },
        { id: 'cursor', name: 'Cursor' },
      ]
    case 'plugin_file_dialog':
      return '/demo/background.svg'
    case 'remote_connection_config': {
      const connection = demoConnections.find((item) => item.id === a.id)
      if (!connection) throw new Error('connection not found')
      return { ...connection, settings: { ...demoConnectionSettings[connection.id] } }
    }
    case 'save_remote_connection': {
      const input = a.input as RemoteConnection & {
        settings: Record<string, string | number | boolean>
      }
      const existing = demoConnections.find((item) => item.id === input.id)
      if (input.id && !existing) throw new Error('connection not found')
      const connection: RemoteConnection = existing || {
        ...input,
        id: crypto.randomUUID(),
        mountPath: '',
        saved: true,
        autostart: false,
      }
      connection.name = input.name
      connection.mountPath ||= 'fiex-terminal://' + connection.id
      demoConnectionSettings[connection.id] = { ...input.settings }
      if (!existing) demoConnections.push(connection)
      return connection
    }
    case 'connection_open': {
      const connection = demoConnections.find((item) => item.id === a.id)
      if (!connection) throw new Error('connection not found')
      return connection.kind === 'ssh'
        ? { kind: 'terminal', connectionId: connection.id }
        : { kind: 'vfs', path: connection.mountPath }
    }
    case 'remote_connections':
      return demoConnections
    case 'update_connection_session': {
      const connection = demoConnections.find((item) => item.id === String(a.id || ''))
      if (!connection) throw new Error('connection not found')
      if (typeof a.saved === 'boolean') connection.saved = a.saved
      if (typeof a.autostart === 'boolean') connection.autostart = a.autostart
      return connection
    }

    default:
      return notHandled
  }
}

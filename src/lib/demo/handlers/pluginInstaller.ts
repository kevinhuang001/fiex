import { demoPlugins, marketplace } from '../plugins'
import { notHandled } from './shared'
const preparedInstalls = new Map<
  string,
  {
    id: string
    name: string
    version: string
    description?: string
    author?: { name: string }
    permissions: string[]
    requestedNetworkDomains: string[]
    extensions?: Record<string, unknown>
  }
>()
export function handlePluginInstaller(command: string, a: Record<string, unknown>): unknown {
  switch (command) {
    case 'plugin_prepare_local_install': {
      preparedInstalls.set('demo-local-install', {
        id: 'org.example.local',
        name: '本地示例插件',
        version: '1.2.3',
        permissions: ['folders.read', 'files.read'],
        requestedNetworkDomains: [],
        author: { name: 'Example Author' },
        extensions: {},
      })
      return {
        ticket: 'demo-local-install',
        id: 'org.example.local',
        name: '本地示例插件',
        version: '1.2.3',
        author: { name: 'Example Author' },
        permissions: ['folders.read', 'files.read'],
        requestedNetworkDomains: [],
        commands: [],
        signed: true,
        trusted: false,
        publisher: 'example',
        publisherName: 'Example Publisher',
        keyId: 'demo-key',
        fingerprint: 'AB12 CD34 EF56 7890',
      }
    }
    case 'plugin_files_dialog':
      return ['/Users/kevin/Documents/Notes.md']
    case 'plugin_network_domains':
      return ['github.com']
    case 'plugin_permission_allowed':
      return new URLSearchParams(location.search).has('pluginFixture')
    case 'plugin_marketplace':
      return marketplace
    case 'plugin_release_assets':
      return marketplace.map((entry) => entry.asset)
    case 'plugin_prepare_install': {
      const entry = marketplace.find((item) => item.asset === a.assetName)
      if (!entry) throw new Error('插件包不存在')
      const ticket = entry.id
      preparedInstalls.set(ticket, entry)
      return {
        ticket,
        id: entry.id,
        name: entry.name,
        version: entry.version,
        description: entry.description,
        icon: 'puzzle',
        author: entry.author,
        permissions: entry.permissions,
        requestedNetworkDomains: entry.requestedNetworkDomains,
        commands: [],
        repository: entry.repository,
        signed: true,
        trusted: true,
        publisher: 'org.fiex',
        publisherName: 'Fiex Contributors',
        keyId: 'demo',
        fingerprint: 'DEMO OFFICIAL KEY',
      }
    }
    case 'plugin_confirm_install': {
      const entry = preparedInstalls.get(String(a.ticket))
      if (!entry) throw new Error('安装预览已过期')
      preparedInstalls.delete(String(a.ticket))
      if (!demoPlugins.some((plugin) => plugin.id === entry.id)) {
        demoPlugins.push({
          id: entry.id,
          name: entry.name,
          version: entry.version,
          description: entry.description,
          enabled: false,
          approved: true,
          backendAvailable: true,

          declaredPermissions: entry.permissions,
          grantedPermissions: [],
          requestedNetworkDomains: entry.requestedNetworkDomains,
          allowedNetworkDomains: [],
          author: entry.author,
          extensions: entry.extensions || {},
        })
      }
      return entry.id
    }
    case 'plugin_cancel_install':
      preparedInstalls.delete(String(a.ticket))
      return null
    case 'plugin_uninstall': {
      const index = demoPlugins.findIndex((plugin) => plugin.id === a.pluginId)
      if (index >= 0) demoPlugins.splice(index, 1)
      return null
    }
    default:
      return notHandled
  }
}

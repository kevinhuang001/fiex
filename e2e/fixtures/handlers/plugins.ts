import type { Entry, EntryDecoration } from '@/lib/types'
import { fixtureDecorations } from '../presentation/decorations'
import { fixturePlugins, officialMode, query } from '../plugins'
import { applyFixturePermissionResponse } from '../permission'
import { notHandled, state } from './shared'
import { handleProviders } from './providers'
import { handlePluginInstaller } from './pluginInstaller'
import { handlePluginMenu } from './plugins/menu'
import { handlePluginInvoke } from './plugins/invoke'
import { handlePluginPreview } from './plugins/preview'
import { pluginSettings } from './plugins/state'

export async function handlePlugins(command: string, a: Record<string, unknown>): Promise<unknown> {
  const provider = handleProviders(command, a)
  if (provider !== notHandled) return provider
  const installed = await handlePluginInstaller(command, a)
  if (installed !== notHandled) return installed
  for (const handler of [handlePluginMenu, handlePluginPreview, handlePluginInvoke]) {
    const result = await handler(command, a)
    if (result !== notHandled) return result
  }
  switch (command) {
    case 'plugins_list':
      return structuredClone(fixturePlugins)
    case 'plugin_logs':
      return { entries: [], cursor: Number(a.after) || 0, hasMore: false }
    case 'plugin_logs_clear':
      return null
    case 'plugin_errors':
      return []
    case 'ui_error_report':
      return null
    case 'plugin_error_clear':
      return null
    case 'plugin_panel_html': {
      if (officialMode) {
        const slug =
          String(a.pluginId || '')
            .split('.')
            .pop() || ''
        const response = await fetch(`/__fiex_official_plugin/${slug}`)
        if (!response.ok) throw new Error(await response.text())
        return await response.text()
      }
      if (query.has('pluginFixture'))
        return "<html><body><h2>示例插件面板</h2><div id=\"selected\">连接中</div>\n<script>\nwindow.addEventListener('message', (event) => {\n if(event.data?.type==='fiex.bridge.probe'){window.parent.postMessage({type:'fiex.bridge.listening'},'*');return;}\n if(event.data?.type!=='fiex.bridge.init'||!event.ports?.[0])return;\n const port=event.ports[0];\n port.onmessage=(message)=>{\n   if(message.data?.type==='response') {\n     const p=(message.data?.result?.targets||[]).map(t=>t.metadata?.name||'');\n     document.getElementById('selected').textContent=p.join(',');\n   }\n };\n port.start();\n port.postMessage({type:'fiex.bridge.ready',bridgeVersion:'1.0',sessionId:event.data.sessionId,nonce:event.data.nonce});\n port.postMessage({type:'request',bridgeVersion:'1.0',sessionId:event.data.sessionId,requestId:'fixture-1',method:'context.get',params:{}});\n});\n</script></body></html>"
      throw new Error('No installed fixture plugin')
    }
    case 'plugin_describe_resources':
      return (a.paths as string[]).map((path) => {
        const entry = state.entries.find((entry) => entry.path === path)
        return {
          ref: { mountId: 'demo', nodeId: crypto.randomUUID() },
          metadata: {
            name: entry?.name || path.split('/').pop() || '',
            kind: entry?.isDir ? 'directory' : 'file',
            extension: entry?.extension || '',
            size: entry?.size || 0,
          },
        }
      })
    case 'plugin_decorations': {
      const entries = (a.entries || []) as Entry[]
      const plugins: EntryDecoration[] =
        !officialMode || a.includePlugins === false
          ? []
          : entries.flatMap((entry) => {
              const extension = (entry.extension || '').toLowerCase()
              if (entry.path.endsWith('/Notes.md'))
                return [
                  {
                    path: entry.path,
                    fields: {
                      name: {
                        badges: [{ text: 'M', color: 'orange', tooltip: 'Git：已修改' }],
                        style: { color: '#c2710c', fontWeight: '600' },
                      },
                      modified: { style: { color: '#c2710c' } },
                      size: { badges: [{ text: '新', color: 'green' }] },
                      kind: { style: { color: '#2f6f9f' } },
                      row: { style: { backgroundLight: '#fff9f0', backgroundDark: '#2b2118' } },
                    },
                    icon: 'fiex-resource://org.example.icon-theme/markdown',
                    iconPath: '/plugin-icons/plugin-markdown.svg',
                    providers: ['Git'],
                  },
                ]
              if (['zip', 'rar', '7z', 'gz', 'bz2', 'xz', 'tar'].includes(extension))
                return [
                  {
                    path: entry.path,
                    fields: {
                      name: {
                        badges: [{ text: extension.toUpperCase().slice(0, 3), color: 'neutral' }],
                      },
                    },
                    providers: ['Archive'],
                  },
                ]
              return []
            })
      return fixtureDecorations(entries, plugins)
    }
    case 'plugin_file_dialog': {
      const extensions = Array.isArray(a.extensions) ? a.extensions : []
      if (a.mode === 'openFile' && extensions.includes('png')) {
        return '/demo/dog.svg'
      }
      if (a.mode === 'saveFile') return '/Users/kevin/Documents/Archive.zip'
      if (a.mode === 'openFile') return '/Users/kevin/Documents/Archive.zip'
      return '/Users/kevin/Documents'
    }
    case 'plugin_activate': {
      const plugin = fixturePlugins.find((item) => item.id === a.pluginId)
      const consent = a.consent as {
        permissions?: string[]
        networkAll?: boolean
        domains?: string[]
        commands?: string[]
      }
      if (plugin) {
        const permissions = consent.permissions || []
        const domains = consent.domains || []
        const commands = consent.commands || []
        const declared = new Set(plugin.declaredPermissions || [])
        const requestedDomains = new Set(plugin.requestedNetworkDomains || [])
        const requestedCommands = new Set(plugin.requestedCommands || [])
        const invalid =
          permissions.some((value) => !declared.has(value)) ||
          (Boolean(consent.networkAll) && !plugin.requestedNetworkAll) ||
          domains.some((value) => !requestedDomains.has(value)) ||
          commands.some((value) => !requestedCommands.has(value)) ||
          ((Boolean(consent.networkAll) || domains.length > 0) &&
            !permissions.includes('network')) ||
          (commands.length > 0 && !permissions.includes('commands'))
        if (invalid) throw new Error('权限选择与插件声明不一致，请重新审核')
        plugin.approved = true
        plugin.enabled = true
        // Activation is partial: keep only what the user selected.
        plugin.grantedPermissions = [...permissions]
        plugin.allowedNetworkAll = Boolean(consent.networkAll)
        plugin.allowedNetworkDomains = [...domains]
        plugin.allowedCommands = [...commands]
      }
      return null
    }
    case 'plugin_set_enabled': {
      const plugin = fixturePlugins.find((item) => item.id === a.pluginId)
      if (plugin) plugin.enabled = Boolean(a.enabled)
      return null
    }
    case 'plugin_set_permission': {
      const plugin = fixturePlugins.find((item) => item.id === a.pluginId)
      if (plugin) {
        const permission = String(a.permission)
        // Mirrors Core: revoking one capability does not deactivate the plugin;
        // the next operation asks again through the Core-owned prompt.
        plugin.grantedPermissions = a.granted
          ? [...new Set([...plugin.grantedPermissions, permission])]
          : plugin.grantedPermissions.filter((value) => value !== permission)
      }
      return null
    }
    case 'plugin_permission_respond':
      applyFixturePermissionResponse(String(a.requestId || ''), Boolean(a.allow), fixturePlugins)
      return null
    case 'plugin_set_command': {
      const plugin = fixturePlugins.find((item) => item.id === a.pluginId)
      if (plugin) {
        plugin.allowedCommands = a.granted
          ? [...new Set([...(plugin.allowedCommands || []), String(a.command)])]
          : (plugin.allowedCommands || []).filter((value) => value !== a.command)
      }
      return null
    }
    case 'plugin_set_network_all': {
      const plugin = fixturePlugins.find((entry) => entry.id === String(a.pluginId || ''))
      if (!plugin) throw new Error('plugin not found')
      plugin.allowedNetworkAll = Boolean(a.all)
      return null
    }
    case 'plugin_set_network_domain': {
      const plugin = fixturePlugins.find((item) => item.id === a.pluginId)
      if (plugin) {
        const domain = String(a.domain)
        plugin.allowedNetworkDomains = a.granted
          ? [...new Set([...plugin.allowedNetworkDomains, domain])]
          : plugin.allowedNetworkDomains.filter((value) => value !== domain)
      }
      return null
    }
    case 'plugin_approve_install': {
      const plugin = fixturePlugins.find((item) => item.id === a.pluginId)
      if (plugin) {
        plugin.approved = true
        plugin.enabled = false
        plugin.grantedPermissions = []
      }
      return null
    }
    case 'plugin_storage_get':
      return (
        pluginSettings.get(String(a.pluginId) + ':' + String(a.scope) + ':' + String(a.key)) ?? null
      )
    case 'plugin_storage_set':
      pluginSettings.set(String(a.pluginId) + ':' + String(a.scope) + ':' + String(a.key), a.value)
      return null
    case 'plugin_storage_delete':
      pluginSettings.delete(String(a.pluginId) + ':' + String(a.scope) + ':' + String(a.key))
      return null
    case 'plugin_secret_set':
      return null
    case 'plugin_secret_available':
      return true
    case 'connection_open': {
      if (String(a.id || '') === 'demo-ssh') {
        return {
          kind: 'pluginView',
          pluginId: 'org.example.terminal',
          connectionId: 'demo-ssh',
          assetPath: '/demo/connection.html',
          state: {},
        }
      }
      // The browser fixture has one WebDAV connection and it registers a VFS,
      // so Core's two-branch navigation resolves to the remote Explorer root.
      if (String(a.id || '') === 'demo-webdav') {
        return { kind: 'vfs', path: 'fiex://mount/demo-webdav/' }
      }
      throw new Error('Unknown demo connection')
    }

    default:
      return notHandled
  }
}

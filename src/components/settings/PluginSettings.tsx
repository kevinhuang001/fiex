import { useEffect, useMemo, useState } from 'react'
import { call } from '@/lib/api'
import { asList } from './plugins/listValue'
import { CommandDialog, DomainDialog } from './plugins/PermissionDialogs'
import {
  PluginImportDialog,
  PluginMessage,
  PluginUninstallDialog,
  type LocalPreview,
} from './plugins/PluginAlerts'
import { PluginList } from './plugins/PluginList'
import { PluginPermissionsSheet } from './plugins/PluginPermissionsSheet'
import { PluginSettingsSheet } from './plugins/PluginSettingsSheet'
import { PluginStore, PluginStoreToolbar } from './plugins/PluginStore'
import { PluginCategoryFilter } from './plugins/PluginCategoryFilter'
import {
  pluginCategories,
  refreshPluginCatalog,
  usePluginCatalog,
  type PluginInfo,
} from '@/lib/plugins'
import { PluginActivationDialog } from './PluginActivationDialog'
import { PluginPermissionPrompt, promptForPermission } from './PluginPermissionPrompt'

export function PluginSettings() {
  const plugins = usePluginCatalog()
  const [category, setCategory] = useState('all')
  const categories = useMemo(() => {
    const counts = new Map<string, number>()
    for (const plugin of plugins) {
      for (const id of pluginCategories(plugin.extensions, plugin.requestedCommands)) {
        counts.set(id, (counts.get(id) || 0) + 1)
      }
    }
    return [...counts].map(([id, count]) => ({ id, count }))
  }, [plugins])
  const shown = useMemo(
    () =>
      category === 'all'
        ? plugins
        : plugins.filter((plugin) =>
            pluginCategories(plugin.extensions, plugin.requestedCommands).includes(category),
          ),
    [plugins, category],
  )
  const [storeOpen, setStoreOpen] = useState(false)
  const [storeCategory, setStoreCategory] = useState('all')
  useEffect(() => {
    const openStore = (event: Event) => {
      const detail = (event as CustomEvent<{ category?: string }>).detail
      const next = detail?.category || 'connections'
      sessionStorage.removeItem('fiex:open-plugin-store')
      setCategory(next)
      setStoreCategory(next)
      setStoreOpen(true)
    }
    const pending = sessionStorage.getItem('fiex:open-plugin-store')
    if (pending) {
      sessionStorage.removeItem('fiex:open-plugin-store')
      setCategory(pending)
      setStoreCategory(pending)
      setStoreOpen(true)
    }
    window.addEventListener('fiex:open-plugin-store', openStore)
    return () => window.removeEventListener('fiex:open-plugin-store', openStore)
  }, [])
  const [localPreview, setLocalPreview] = useState<LocalPreview | null>(null)
  const [permissionsFor, setPermissionsFor] = useState<PluginInfo | null>(null)
  const [settingsFor, setSettingsFor] = useState<PluginInfo | null>(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [draft, setDraft] = useState<Record<string, string | boolean | string[]>>({})
  const [permissionToApprove, setPermissionToApprove] = useState<string | null>(null)
  useEffect(() => {
    setPermissionsFor((current) =>
      current ? plugins.find((plugin) => plugin.id === current.id) || null : null,
    )
    setSettingsFor((current) =>
      current ? plugins.find((plugin) => plugin.id === current.id) || null : null,
    )
  }, [plugins])
  const [domainDraft, setDomainDraft] = useState('')
  const [domainSettingsOpen, setDomainSettingsOpen] = useState(false)
  const [domainToApprove, setDomainToApprove] = useState<string | null>(null)
  const [commandDraft, setCommandDraft] = useState('')
  const [commandSettingsOpen, setCommandSettingsOpen] = useState(false)
  const [activationFor, setActivationFor] = useState<PluginInfo | null>(null)
  const [commandToApprove, setCommandToApprove] = useState<string | null>(null)
  const [confirmRemove, setConfirmRemove] = useState<PluginInfo | null>(null)
  const chooseLocalPlugin = async () => {
    setBusy(true)
    setMessage('')
    try {
      const path = await call<string | null>('plugin_file_dialog', {
        mode: 'openFile',
        extensions: ['fiexplugin'],
      })
      if (!path) return
      const preview = await call<LocalPreview>('plugin_prepare_local_install', { path })
      setLocalPreview(preview)
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const installLocal = async () => {
    if (!localPreview) return
    setBusy(true)
    try {
      await call('plugin_confirm_install', {
        ticket: localPreview.ticket,
        allowUntrusted: !localPreview.trusted,
      })
      setLocalPreview(null)
      refreshPluginCatalog()
      setMessage(`已导入 ${localPreview.name} ${localPreview.version}`)
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const cancelLocalImport = () => {
    if (localPreview) void call('plugin_cancel_install', { ticket: localPreview.ticket })
    setLocalPreview(null)
  }

  const fields = settingsFor?.extensions?.settings || []
  useEffect(() => {
    if (!settingsFor) return
    let alive = true
    setMessage('')
    void Promise.all(
      fields.map(async (field) => {
        if (field.type === 'secret') return [field.id, ''] as const
        const value = await call<unknown>('plugin_storage_get', {
          pluginId: settingsFor.id,
          scope: 'global',
          key: field.id,
        })
        if (field.type === 'list') return [field.id, asList(value, field)] as const
        return [
          field.id,
          (value ?? field.defaultValue ?? (field.type === 'checkbox' ? false : '')) as
            string | boolean,
        ] as const
      }),
    )
      .then((entries) => {
        if (alive) setDraft(Object.fromEntries(entries))
      })
      .catch((error) => {
        if (alive) setMessage(String(error))
      })
    return () => {
      alive = false
    }
    // Field definitions are manifest data; refetch when the plugin changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settingsFor?.id])

  const updateActivation = async (plugin: PluginInfo, enabled: boolean) => {
    if (enabled) {
      setActivationFor(plugin)
      return
    }
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_enabled', { pluginId: plugin.id, enabled })
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const updatePermission = async (plugin: PluginInfo, permission: string, granted: boolean) => {
    if (granted) {
      setPermissionToApprove(permission)
      return
    }
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_permission', { pluginId: plugin.id, permission, granted })
      setPermissionToApprove(null)
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const grantPermission = async (plugin: PluginInfo) => {
    if (!permissionToApprove) return
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_permission', {
        pluginId: plugin.id,
        permission: permissionToApprove,
        granted: true,
      })
      setPermissionToApprove(null)
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const updateDomain = async (plugin: PluginInfo, domain: string, granted: boolean) => {
    if (granted && domainToApprove !== domain) {
      setDomainToApprove(domain)
      return
    }
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_network_domain', { pluginId: plugin.id, domain, granted })
      setDomainToApprove(null)
      setDomainDraft('')
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const updateNetworkAll = async (plugin: PluginInfo, all: boolean) => {
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_network_all', { pluginId: plugin.id, all })
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const updateCommand = async (plugin: PluginInfo, command: string, granted: boolean) => {
    if (granted && commandToApprove !== command) {
      setCommandToApprove(command)
      return
    }
    setBusy(true)
    setMessage('')
    try {
      await call('plugin_set_command', { pluginId: plugin.id, command, granted })
      setCommandDraft('')
      setCommandToApprove(null)
      refreshPluginCatalog()
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const uninstall = async () => {
    if (!confirmRemove || busy) return
    setBusy(true)
    try {
      await call('plugin_uninstall', { pluginId: confirmRemove.id })
      setConfirmRemove(null)
      setPermissionsFor(null)
      refreshPluginCatalog()
      setMessage('')
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const saveSettings = async () => {
    if (!settingsFor || busy) return
    setBusy(true)
    setMessage('')
    try {
      for (const field of fields) {
        const value = draft[field.id]
        if (field.type === 'secret') {
          if (typeof value === 'string' && value.length)
            await call('plugin_secret_set', {
              pluginId: settingsFor.id,
              scope: 'global',
              key: field.id,
              value,
            })
        } else {
          await call('plugin_storage_set', {
            pluginId: settingsFor.id,
            scope: 'global',
            key: field.id,
            value:
              field.type === 'number'
                ? Number(value)
                : field.type === 'list'
                  ? (Array.isArray(value) ? value : []).map((item) => item.trim()).filter(Boolean)
                  : value,
          })
        }
      }
      setMessage('插件设置已保存')
      setDraft((old) =>
        Object.fromEntries(
          Object.entries(old).map(([key, value]) =>
            fields.some((f) => f.id === key && f.type === 'secret') ? [key, ''] : [key, value],
          ),
        ),
      )
    } catch (error) {
      setMessage(String(error))
    } finally {
      setBusy(false)
    }
  }
  const updateDraft = (fieldId: string, value: string | boolean | string[]) =>
    setDraft((old) => ({ ...old, [fieldId]: value }))

  return (
    <>
      <div className="plugin-page">
        <PluginStoreToolbar
          onImport={() => void chooseLocalPlugin()}
          onOpenStore={() => setStoreOpen(true)}
        >
          <PluginCategoryFilter categories={categories} value={category} onChange={setCategory} />
        </PluginStoreToolbar>
        <PluginList
          plugins={shown}
          busy={busy}
          onOpenSettings={(plugin) => {
            setMessage('')
            setSettingsFor(plugin)
          }}
          onOpenDetails={(plugin) => {
            setMessage('')
            setDomainSettingsOpen(false)
            setCommandSettingsOpen(false)
            setPermissionsFor(plugin)
          }}
          onRemove={(plugin) => {
            setMessage('')
            setConfirmRemove(plugin)
          }}
          onToggleActivation={(plugin, enabled) => void updateActivation(plugin, enabled)}
          onBrowseStore={() => setStoreOpen(true)}
        />
        <PluginMessage message={message} />
      </div>

      <PluginStore
        open={storeOpen}
        installed={plugins}
        initialCategory={storeCategory}
        onClose={() => setStoreOpen(false)}
        onInstalled={() => {
          refreshPluginCatalog()
          setMessage('插件已安装，尚未激活。点击激活后可逐项选择权限。')
        }}
      />

      <PluginPermissionsSheet
        plugin={permissionsFor}
        busy={busy}
        message={message}
        onClose={() => {
          setPermissionsFor(null)
          setDomainSettingsOpen(false)
          setCommandSettingsOpen(false)
          setMessage('')
        }}
        onActivate={setActivationFor}
        onTogglePermission={(plugin, permission, granted) =>
          void updatePermission(plugin, permission, granted)
        }
        onConfigure={(permission) => {
          setMessage('')
          if (permission === 'network') setDomainSettingsOpen(true)
          else setCommandSettingsOpen(true)
        }}
      />

      {permissionsFor &&
        domainSettingsOpen &&
        permissionsFor.declaredPermissions.includes('network') && (
          <DomainDialog
            plugin={permissionsFor}
            busy={busy}
            draft={domainDraft}
            onDraftChange={setDomainDraft}
            onToggle={(domain, granted) => void updateDomain(permissionsFor, domain, granted)}
            onAdd={(domain) => void updateDomain(permissionsFor, domain, true)}
            onToggleAll={(all) => void updateNetworkAll(permissionsFor, all)}
            onClose={() => setDomainSettingsOpen(false)}
          />
        )}
      {permissionsFor &&
        commandSettingsOpen &&
        permissionsFor.declaredPermissions.includes('commands') && (
          <CommandDialog
            plugin={permissionsFor}
            busy={busy}
            draft={commandDraft}
            onDraftChange={setCommandDraft}
            onToggle={(command, granted) => void updateCommand(permissionsFor, command, granted)}
            onAdd={(command) => void updateCommand(permissionsFor, command, true)}
            onClose={() => setCommandSettingsOpen(false)}
          />
        )}

      <PluginSettingsSheet
        plugin={settingsFor}
        fields={fields}
        draft={draft}
        busy={busy}
        message={message}
        onDraftChange={updateDraft}
        onSave={() => void saveSettings()}
        onClose={() => setSettingsFor(null)}
        onDismiss={() => {
          setSettingsFor(null)
          setMessage('')
        }}
      />

      {permissionsFor && domainToApprove && (
        <PluginPermissionPrompt
          open
          pluginName={permissionsFor.name}
          icon={permissionsFor.icon}
          item={{
            id: `domain:${domainToApprove}`,
            kind: 'network',
            title: '网络域名',
            value: domainToApprove,
          }}
          step={1}
          total={1}
          busy={busy}
          onAllow={() => void updateDomain(permissionsFor, domainToApprove, true)}
          onDeny={() => setDomainToApprove(null)}
        />
      )}
      {permissionsFor && permissionToApprove && (
        <PluginPermissionPrompt
          open
          pluginName={permissionsFor.name}
          icon={permissionsFor.icon}
          item={promptForPermission(permissionToApprove)}
          step={1}
          total={1}
          busy={busy}
          onAllow={() => void grantPermission(permissionsFor)}
          onDeny={() => setPermissionToApprove(null)}
        />
      )}

      <PluginUninstallDialog
        plugin={confirmRemove}
        busy={busy}
        onClose={() => setConfirmRemove(null)}
        onConfirm={() => void uninstall()}
      />
      <PluginImportDialog
        preview={localPreview}
        busy={busy}
        onClose={() => setLocalPreview(null)}
        onCancel={cancelLocalImport}
        onConfirm={() => void installLocal()}
      />
      {activationFor && (
        <PluginActivationDialog
          key={activationFor.id}
          plugin={activationFor}
          onClose={() => setActivationFor(null)}
          onActivated={() => {
            setActivationFor(null)
            refreshPluginCatalog()
            setMessage('插件已激活，可随时在插件详情中撤销授权。')
          }}
        />
      )}
      {permissionsFor && commandToApprove && (
        <PluginPermissionPrompt
          open
          pluginName={permissionsFor.name}
          icon={permissionsFor.icon}
          item={{
            id: `command:${commandToApprove}`,
            kind: 'command',
            title: '外部命令',
            value: commandToApprove,
          }}
          step={1}
          total={1}
          busy={busy}
          onAllow={() => void updateCommand(permissionsFor, commandToApprove, true)}
          onDeny={() => setCommandToApprove(null)}
        />
      )}
    </>
  )
}

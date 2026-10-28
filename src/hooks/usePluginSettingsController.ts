import { getLocale } from '@/i18n'
import { useEffect, useMemo, useState } from 'react'
import { call, demo } from '@/lib/api'
import { usePluginConfiguration } from './usePluginConfiguration'
import type { LocalPreview } from '@/components/settings/plugins/PluginAlerts'
import { openFilePicker } from '@/lib/filePicker'
import { requestPluginActivation } from '@/components/plugins/PluginActivationHost'
import {
  pluginCategories,
  refreshPluginCatalog,
  usePluginCatalog,
  type PluginInfo,
} from '@/lib/plugins'

export function usePluginSettingsController() {
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
  }, [plugins, getLocale()])
  const shown = useMemo(
    () =>
      category === 'all'
        ? plugins
        : plugins.filter((plugin) =>
            pluginCategories(plugin.extensions, plugin.requestedCommands).includes(category),
          ),
    [plugins, category, getLocale()],
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
  const [logsFor, setLogsFor] = useState<PluginInfo | null>(null)
  const [settingsFor, setSettingsFor] = useState<PluginInfo | null>(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
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
  const [commandToApprove, setCommandToApprove] = useState<string | null>(null)
  const [confirmRemove, setConfirmRemove] = useState<PluginInfo | null>(null)
  const chooseLocalPlugin = async () => {
    setBusy(true)
    setMessage('')
    try {
      const picked = demo
        ? '/Users/kevin/Documents/Example.fiexplugin'
        : await openFilePicker({
            mode: 'openFile',
            extensions: ['fiexplugin'],
          })
      const path = Array.isArray(picked) ? picked[0] : picked
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
      const pluginId = await call<string>('plugin_confirm_install', {
        ticket: localPreview.ticket,
        allowUntrusted: !localPreview.trusted,
      })
      await requestPluginActivation(pluginId)
      setLocalPreview(null)
      refreshPluginCatalog()
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

  const { fields, draft, saveSettings, updateDraft } = usePluginConfiguration(
    settingsFor,
    setSettingsFor,
    busy,
    setBusy,
    setMessage,
  )
  const updateActivation = async (plugin: PluginInfo, enabled: boolean) => {
    if (enabled) {
      await requestPluginActivation(plugin.id)
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

  return {
    plugins,
    category,
    setCategory,
    categories,
    shown,
    storeOpen,
    setStoreOpen,
    storeCategory,
    setStoreCategory,
    localPreview,
    setLocalPreview,
    permissionsFor,
    setPermissionsFor,
    logsFor,
    setLogsFor,
    settingsFor,
    setSettingsFor,
    busy,
    setBusy,
    message,
    setMessage,
    draft,
    permissionToApprove,
    setPermissionToApprove,
    domainDraft,
    setDomainDraft,
    domainSettingsOpen,
    setDomainSettingsOpen,
    domainToApprove,
    setDomainToApprove,
    commandDraft,
    setCommandDraft,
    commandSettingsOpen,
    setCommandSettingsOpen,
    commandToApprove,
    setCommandToApprove,
    confirmRemove,
    setConfirmRemove,
    chooseLocalPlugin,
    installLocal,
    cancelLocalImport,
    fields,
    updateActivation,
    updatePermission,
    grantPermission,
    updateDomain,
    updateNetworkAll,
    updateCommand,
    uninstall,
    saveSettings,
    updateDraft,
  }
}

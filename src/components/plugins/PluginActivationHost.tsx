import { useEffect, useState } from 'react'
import { PluginActivationDialog } from '@/components/settings/PluginActivationDialog'
import { refreshPluginCatalog, syncPluginCatalog, usePluginCatalog } from '@/lib/plugins'

/** Install confirmations and explicit activation share one consent queue.
 * Directory discovery only refreshes the catalog; copied packages stay off. */
export async function requestPluginActivation(pluginId: string) {
  await syncPluginCatalog()
  window.dispatchEvent(new CustomEvent('fiex:activate-plugin', { detail: pluginId }))
}

export function PluginActivationHost() {
  const plugins = usePluginCatalog()
  const [queue, setQueue] = useState<string[]>([])
  useEffect(() => {
    const request = (event: Event) => {
      const id = (event as CustomEvent<string>).detail
      setQueue((current) => (current.includes(id) ? current : [...current, id]))
      refreshPluginCatalog()
    }
    window.addEventListener('fiex:activate-plugin', request)
    return () => window.removeEventListener('fiex:activate-plugin', request)
  }, [])
  const plugin = queue.flatMap((id) => plugins.filter((entry) => entry.id === id))[0]
  const close = () => setQueue((current) => current.filter((id) => id !== plugin?.id))
  if (!plugin) return null
  return (
    <PluginActivationDialog
      key={JSON.stringify([
        plugin.id,
        plugin.version,
        plugin.declaredPermissions,
        plugin.requestedNetworkDomains,
        plugin.requestedCommands,
      ])}
      plugin={plugin}
      onClose={close}
      onActivated={() => {
        close()
        refreshPluginCatalog()
      }}
    />
  )
}

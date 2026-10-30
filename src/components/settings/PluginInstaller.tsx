import { requestPluginActivation } from '@/components/plugins/PluginActivationHost'
import { t } from '@/i18n'
import { useEffect, useState } from 'react'
import { call } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { pluginMarketplace, type MarketplacePlugin, type PluginInfo } from '@/lib/plugins'
import { ImportSheet } from './installer/ImportSheet'
import { MarketplaceList } from './installer/MarketplaceList'
import type { InstallPreview } from './installer/types'

export { pluginIcon } from './installer/pluginIcon'

/**
 * App-store style official plugin catalog. The repository and asset names are
 * a fixed release contract: `fiex-catalog.json` plus
 * `<slug>-<platform>.fiexplugin`. Nothing is started until the user reviews the
 * exact permissions and confirms installation.
 */
export function PluginInstaller({
  open,
  installed,
  onClose,
  onInstalled,
  initialCategory,
}: {
  open: boolean
  installed: PluginInfo[]
  onClose: () => void
  onInstalled: () => void
  initialCategory?: string
}) {
  const [entries, setEntries] = useState<MarketplacePlugin[]>([])
  const [loading, setLoading] = useState(false)
  const [busyId, setBusyId] = useState('')
  const [preview, setPreview] = useState<InstallPreview | null>(null)
  const [installing, setInstalling] = useState(false)
  const [message, setMessage] = useState('')
  const [query, setQuery] = useState('')

  const load = () => {
    if (loading) return
    setLoading(true)
    setMessage('')
    void pluginMarketplace()
      .then(setEntries)
      .catch((cause) => reportError(cause, 'plugins:marketplace'))
      .finally(() => setLoading(false))
  }
  useEffect(() => {
    if (!open) return
    setPreview(null)
    setMessage('')
    load()
    // Refresh the catalog each time the store opens; there is no manual button.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  const prepare = async (entry: MarketplacePlugin) => {
    if (busyId) return
    setBusyId(entry.id)
    setMessage('')
    try {
      const data = await call<InstallPreview>('plugin_prepare_install', {
        repository: entry.repository,
        assetName: entry.asset,
      })
      setPreview(data)
    } catch (cause) {
      reportError(cause, 'plugins:installation')
    } finally {
      setBusyId('')
    }
  }
  const discard = () => {
    const ticket = preview?.ticket
    setPreview(null)
    if (ticket) void call('plugin_cancel_install', { ticket }).catch(() => {})
  }
  const confirm = async () => {
    if (!preview || installing) return
    setInstalling(true)
    try {
      const pluginId = await call<string>('plugin_confirm_install', {
        ticket: preview.ticket,
        allowUntrusted: !preview.trusted,
      })
      await requestPluginActivation(pluginId)
      const installed = preview
      setPreview(null)
      setMessage(
        installed.isUpdate
          ? t('{0} 已更新到 {1}', installed.name, installed.version)
          : t('{0} 已安装', installed.name),
      )
      onInstalled()
    } catch (cause) {
      reportError(cause, 'plugins:installation')
    } finally {
      setInstalling(false)
    }
  }

  return (
    <>
      <MarketplaceList
        open={open}
        installed={installed}
        onClose={onClose}
        query={query}
        setQuery={setQuery}
        message={message}
        loading={loading}
        entries={entries}
        busyId={busyId}
        onPrepare={prepare}
        initialCategory={initialCategory}
      />
      <ImportSheet
        preview={preview}
        installing={installing}
        onDiscard={discard}
        onConfirm={confirm}
      />
    </>
  )
}

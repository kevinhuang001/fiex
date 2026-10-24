import { useEffect, useState } from 'react'
import { call } from '@/lib/api'
import { pluginMarketplace, type MarketplacePlugin } from '@/lib/plugins'
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
  installedIds,
  onClose,
  onInstalled,
}: {
  open: boolean
  installedIds: string[]
  onClose: () => void
  onInstalled: () => void
}) {
  const [entries, setEntries] = useState<MarketplacePlugin[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [busyId, setBusyId] = useState('')
  const [preview, setPreview] = useState<InstallPreview | null>(null)
  const [installing, setInstalling] = useState(false)
  const [message, setMessage] = useState('')
  const [query, setQuery] = useState('')

  const load = () => {
    if (loading) return
    setLoading(true)
    setError('')
    setMessage('')
    void pluginMarketplace()
      .then(setEntries)
      .catch((cause) => setError(String(cause)))
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
    setError('')
    setMessage('')
    try {
      const data = await call<InstallPreview>('plugin_prepare_install', {
        repository: entry.repository,
        assetName: entry.asset,
      })
      setPreview(data)
    } catch (cause) {
      setError(String(cause))
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
    setError('')
    try {
      await call('plugin_confirm_install', {
        ticket: preview.ticket,
        allowUntrusted: !preview.trusted,
      })
      const installed = preview
      setPreview(null)
      setMessage(`${installed.name} 已安装`)
      onInstalled()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setInstalling(false)
    }
  }

  return (
    <>
      <MarketplaceList
        open={open}
        installedIds={installedIds}
        onClose={onClose}
        query={query}
        setQuery={setQuery}
        error={error}
        message={message}
        loading={loading}
        entries={entries}
        busyId={busyId}
        onPrepare={prepare}
      />
      <ImportSheet
        preview={preview}
        error={error}
        installing={installing}
        onDiscard={discard}
        onConfirm={confirm}
      />
    </>
  )
}

import { useState } from 'react'
import { call } from '@/lib/api'
import type { PluginInfo } from '@/lib/plugins'
import { activationConsent } from '@/lib/pluginPermissions'
import {
  PluginPermissionPrompt,
  activationPromptItems,
} from '@/components/settings/PluginPermissionPrompt'

/**
 * Mobile-style activation: every declared permission, network host and command
 * is requested one at a time. Core is only called after the final request is
 * allowed, so denying or closing any prompt grants nothing.
 */
export function PluginActivationDialog({
  plugin,
  onClose,
  onActivated,
}: {
  plugin: PluginInfo
  onClose: () => void
  onActivated: () => void
}) {
  const [items] = useState(() => activationPromptItems(plugin))
  const [index, setIndex] = useState(0)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const item = items[Math.min(index, items.length - 1)]
  const activate = async () => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      await call('plugin_activate', { pluginId: plugin.id, consent: activationConsent(plugin) })
      onActivated()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }
  const allow = () => {
    if (busy) return
    setError('')
    if (index + 1 < items.length) {
      setIndex(index + 1)
      return
    }
    void activate()
  }
  return (
    <PluginPermissionPrompt
      open
      pluginName={plugin.name}
      icon={plugin.icon}
      item={item}
      step={index + 1}
      total={items.length}
      busy={busy}
      error={error}
      allowLabel={index + 1 < items.length ? '允许' : '允许并激活'}
      onAllow={allow}
      onDeny={() => {
        if (!busy) onClose()
      }}
    />
  )
}

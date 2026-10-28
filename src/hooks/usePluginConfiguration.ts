import { useEffect, useState } from 'react'
import { t } from '@/i18n'
import { call } from '@/lib/api'
import type { PluginInfo } from '@/lib/plugins'
import { asList } from '@/components/settings/plugins/listValue'

export function usePluginConfiguration(
  settingsFor: PluginInfo | null,
  setSettingsFor: (plugin: PluginInfo | null) => void,
  busy: boolean,
  setBusy: (busy: boolean) => void,
  setMessage: (message: string) => void,
) {
  const [draft, setDraft] = useState<Record<string, string | boolean | string[]>>({})
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
      setMessage(t('插件设置已保存'))
      setSettingsFor(null)
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

  return { fields, draft, saveSettings, updateDraft }
}

import { useEffect, useState } from 'react'
import { t } from '@/i18n'
import { call } from '@/lib/api'
import { Check, Database, KeyRound, LoaderCircle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import type { PluginField, PluginInfo } from '@/lib/plugins'
import { PluginListField } from './PluginListField'
import { asList } from './listValue'

/**
 * Manifest-declared settings for one plugin. `onDismiss` closes the modal and
 * clears the page message; the footer 关闭 only hides the modal, so a saved
 * message stays visible on the page.
 */
export function PluginSettingsSheet({
  plugin,
  fields,
  draft,
  busy,
  onDraftChange,
  onSave,
  onClose,
  onDismiss,
}: {
  plugin: PluginInfo | null
  fields: PluginField[]
  draft: Record<string, string | boolean | string[]>
  busy: boolean
  onDraftChange: (fieldId: string, value: string | boolean | string[]) => void
  onSave: () => void
  onClose: () => void
  onDismiss: () => void
}) {
  const [fonts, setFonts] = useState<string[]>([])
  useEffect(() => {
    if (!fields.some((field) => field.type === 'font')) return
    let alive = true
    void call<string[]>('system_fonts')
      .then((values) => {
        if (alive) setFonts(values)
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [fields])
  return (
    <Modal
      open={Boolean(plugin)}
      onClose={onDismiss}
      title={plugin ? t('{0} · 设置', plugin.name) : t('插件设置')}
    >
      <div className="plugin-settings-fields">
        {fields.map((field: PluginField) => {
          const label = (
            <span className="plugin-settings-label">
              {field.type === 'secret' ? <KeyRound size={14} /> : <Database size={14} />}
              {field.label}
            </span>
          )
          if (field.type === 'list')
            return (
              <div key={field.id} className="plugin-settings-field">
                {label}
                <PluginListField
                  field={field}
                  values={asList(draft[field.id], field)}
                  onChange={(values) => onDraftChange(field.id, values)}
                />
              </div>
            )
          return (
            <label key={field.id} className="plugin-settings-field">
              {label}
              {field.type === 'checkbox' ? (
                <input
                  type="checkbox"
                  checked={Boolean(draft[field.id])}
                  onChange={(event) => onDraftChange(field.id, event.target.checked)}
                />
              ) : field.type === 'font' ? (
                <>
                  <input
                    type="text"
                    list={`plugin-font-${field.id}`}
                    value={String(draft[field.id] ?? '')}
                    placeholder={field.placeholder || t('使用系统主题字体')}
                    onChange={(event) => onDraftChange(field.id, event.target.value)}
                  />
                  <datalist id={`plugin-font-${field.id}`}>
                    {fonts.map((font) => (
                      <option key={font} value={font} />
                    ))}
                  </datalist>
                </>
              ) : field.type === 'color' ? (
                <>
                  <input
                    type="color"
                    value={String(draft[field.id] || '#e6edf3')}
                    onChange={(event) => onDraftChange(field.id, event.target.value)}
                  />
                  <input
                    type="text"
                    value={String(draft[field.id] ?? '')}
                    placeholder={field.placeholder || t('留空跟随主题')}
                    onChange={(event) => onDraftChange(field.id, event.target.value)}
                  />
                </>
              ) : (
                <input
                  type={field.type === 'secret' ? 'password' : field.type}
                  value={String(draft[field.id] ?? '')}
                  placeholder={
                    field.type === 'secret' ? t('保留原有凭据（留空不修改）') : field.placeholder
                  }
                  onChange={(event) => onDraftChange(field.id, event.target.value)}
                />
              )}
            </label>
          )
        })}
        {!fields.length && <p className="sidebar-hint">{t('此插件没有需要配置的设置。')}</p>}
        <footer className="plugin-consent-actions">
          <Button variant="outline" onClick={onClose}>
            {t('关闭')}
          </Button>
          <Button disabled={busy || !fields.length} onClick={onSave}>
            {busy ? <LoaderCircle className="spin" size={14} /> : <Check size={14} />}
            {t('保存插件设置')}
          </Button>
        </footer>
      </div>
    </Modal>
  )
}

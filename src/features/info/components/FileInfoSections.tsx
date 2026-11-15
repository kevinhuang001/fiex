import { Check, Copy } from 'lucide-react'
import { t } from '@/i18n'
import type { InfoField, InfoSection } from '@/lib/ipcValidation/infoTypes'

export function FileInfoSections({
  sections,
  copied,
  computeDisabled,
  onCopy,
  onCompute,
}: {
  sections: InfoSection[]
  copied: string
  computeDisabled: boolean
  onCopy: (value: string, key: string) => void
  onCompute: (section: InfoSection, field: InfoField) => void
}) {
  return (
    <>
      {sections.map((section) => (
        <div className="file-info-provider-section" key={section.id}>
          <h4 className="file-info-provider-title">{section.title}</h4>
          {section.error && (
            <div className="file-info-row">
              <dt>{t('提供程序')}</dt>
              <dd>{section.error}</dd>
            </div>
          )}
          {section.fields.map((field) => {
            const key = JSON.stringify([section.providerId, field.subsectionId, field.id])
            const value =
              field.status === 'ready'
                ? field.value
                : field.status === 'unavailable'
                  ? t('不可用')
                  : field.status === 'error'
                    ? field.error
                    : field.compute === 'onDemand'
                      ? t('尚未计算')
                      : t('正在计算…')
            return (
              <div className="file-info-row" key={JSON.stringify([field.subsectionId, field.id])}>
                <dt>{field.label}</dt>
                <dd>
                  <span>{value}</span>
                  {field.status === 'ready' && field.value && (
                    <button
                      type="button"
                      className="info-copy"
                      title={copied === key ? t('已复制') : t('复制')}
                      aria-label={t('复制')}
                      onClick={() => onCopy(field.value!, key)}
                    >
                      {copied === key ? <Check size={12} /> : <Copy size={12} />}
                    </button>
                  )}
                  {field.compute === 'onDemand' && ['idle', 'error'].includes(field.status) && (
                    <button
                      type="button"
                      className="file-info-action"
                      disabled={computeDisabled}
                      onClick={() => onCompute(section, field)}
                    >
                      {field.status === 'error' ? t('重试并授权') : t('计算')}
                    </button>
                  )}
                  {field.compute === 'onDemand' && field.status === 'pending' && (
                    <button type="button" className="file-info-action" disabled>
                      {t('正在计算…')}
                    </button>
                  )}
                </dd>
              </div>
            )
          })}
        </div>
      ))}
    </>
  )
}

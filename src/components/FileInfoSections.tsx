import { Fragment } from 'react'
import { Check, Copy } from 'lucide-react'
import { t } from '@/i18n'
import type { InfoField, InfoSection } from './FileInfoDialog'

type SectionGroup = {
  id: string
  title: string
  source: InfoSection['source']
  sections: InfoSection[]
}

function groupSections(sections: InfoSection[]): SectionGroup[] {
  const core = sections.filter((section) => section.source === 'core')
  return [
    ...(core.length
      ? [{ id: 'core', title: t('核心信息'), source: 'core' as const, sections: core }]
      : []),
    ...sections
      .filter((section) => section.source === 'plugin')
      .map((section) => ({
        id: section.id,
        title: section.title,
        source: section.source,
        sections: [section],
      })),
  ]
}

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
      {groupSections(sections).map((group) => (
        <Fragment key={group.id}>
          <div className="file-info-group">
            <span>{group.title}</span>
            {group.source === 'plugin' && group.sections[0] && (
              <small>{t('来自：{0}', group.sections[0].provider)}</small>
            )}
          </div>
          {group.sections.map((section) => (
            <Fragment key={section.id}>
              {section.error && (
                <div className="file-info-row">
                  <dt>{t('提供程序')}</dt>
                  <dd>{section.error}</dd>
                </div>
              )}
              {section.fields.map((field) => {
                const key = `${section.providerId}:${field.sectionId}:${field.id}`
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
                  <div className="file-info-row" key={`${field.sectionId}:${field.id}`}>
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
            </Fragment>
          ))}
        </Fragment>
      ))}
    </>
  )
}

import { useEffect, useRef, useState } from 'react'
import { t } from '@/i18n'
import { FileIcon } from '@/shared/ui/FileIcon'
import { Modal } from '@/shared/ui/Modal'
import { tagHex } from '@/lib/tags'
import type { Entry } from '@/lib/types'
import { useInfoSession } from '../hooks/useInfoSession'
import type { InfoSessionServices } from '../types'
import { FileInfoSections } from './FileInfoSections'

export function FileInfoDialog({
  info,
  onClose,
  services,
}: {
  info: Entry | null
  onClose: () => void
  services: InfoSessionServices
}) {
  const [copied, setCopied] = useState('')
  const copiedTimer = useRef<number | null>(null)
  const { sections, loading, computeDisabled, compute } = useInfoSession(info, services)

  useEffect(
    () => () => {
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current)
    },
    [],
  )

  const copy = async (value: string, key: string) => {
    try {
      await navigator.clipboard.writeText(value)
      setCopied(key)
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current)
      copiedTimer.current = window.setTimeout(() => setCopied(''), 1200)
    } catch {
      // Keep the value selectable when clipboard access is unavailable.
    }
  }

  return (
    <Modal open={!!info} onClose={onClose} title={t('显示简介')} className="file-info-modal">
      {info && (
        <div className="file-info">
          <FileIcon entry={info} size={72} />
          <h3>{info.name}</h3>
          <div className="file-info-scroll">
            <dl>
              <FileInfoSections
                sections={sections}
                copied={copied}
                computeDisabled={computeDisabled}
                onCopy={(value, key) => void copy(value, key)}
                onCompute={(section, field) => void compute(section, field)}
              />
            </dl>
            {loading && <p className="file-info-status">{t('正在读取信息…')}</p>}
            {info.tags.length > 0 && (
              <div className="file-info-tags">
                {info.tags.map((tag) => (
                  <span className="tag-chip" key={`${tag.name}:${tag.color}`}>
                    <i style={{ background: tagHex(tag) }} />
                    {tag.name}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </Modal>
  )
}

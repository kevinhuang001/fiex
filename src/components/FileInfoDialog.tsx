import { t } from '@/i18n'
import { Check, Copy } from 'lucide-react'
import { Fragment, useEffect, useRef, useState } from 'react'
import { Modal } from './ui/Modal'
import { FileIcon } from './FileIcon'
import { date, displayAddress, parent, kind, size } from '@/lib/files'
import { tagHex } from '@/lib/tags'
import { call } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import type { Entry } from '@/lib/types'

interface InfoRow {
  label: string
  value: string
  copyable?: boolean
}
interface InfoSection {
  id: string
  source: 'core' | 'plugin'
  title: string
  pluginId?: string
  provider: string
  rows: InfoRow[]
}

export function FileInfoDialog({ info, onClose }: { info: Entry | null; onClose: () => void }) {
  const [sections, setSections] = useState<InfoSection[]>([])
  const [loading, setLoading] = useState(false)
  const [copied, setCopied] = useState('')
  const copiedTimer = useRef<number | null>(null)

  useEffect(
    () => () => {
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current)
    },
    [],
  )

  useEffect(() => {
    if (!info) return
    let alive = true
    setSections([])
    setLoading(true)
    const timeout = new Promise<InfoSection[]>((_, reject) =>
      window.setTimeout(
        () => reject(new Error(t('读取插件信息超时，请检查插件是否正常运行。'))),
        15_000,
      ),
    )
    void Promise.race([call<InfoSection[]>('file_info_details', { path: info.path }), timeout])
      .then((value) => {
        if (alive) setSections(Array.isArray(value) ? value : [])
      })
      .catch((cause) => {
        if (alive) reportError(cause, 'file-info')
      })
      .finally(() => {
        if (alive) setLoading(false)
      })
    return () => {
      alive = false
    }
  }, [info])

  const copy = async (value: string, key: string) => {
    try {
      await navigator.clipboard.writeText(value)
      setCopied(key)
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current)
      copiedTimer.current = window.setTimeout(() => setCopied(''), 1200)
    } catch {
      // Clipboard access is best effort; the value stays selectable.
    }
  }

  const basicRows: InfoRow[] = info
    ? [
        { label: t('种类'), value: kind(info) },
        { label: t('大小'), value: info.isDir ? t('文件夹') : size(info.size) },
        { label: t('位置'), value: displayAddress(parent(info.path)) },
        { label: t('创建时间'), value: date(info.created) },
        { label: t('修改时间'), value: date(info.modified) },
        { label: t('权限'), value: info.readonly ? t('只读') : t('可读写') },
        { label: t('符号链接'), value: info.isSymlink ? t('是') : t('否') },
        ...(info.linkTarget ? [{ label: t('链接到'), value: info.linkTarget }] : []),
      ]
    : []
  // Core's metadata provider owns the basic attributes. Only use the local
  // Entry fallback for old responses or paths Core could not stat.
  const hasCoreMetadata = sections.some((section) => section.id === 'core.metadata')
  const groups: Array<{ title: string; from?: string; rows: InfoRow[] }> = info
    ? [
        ...(hasCoreMetadata ? [] : [{ title: t('文件'), rows: basicRows }]),
        ...sections
          .filter((section) => section.rows.length > 0)
          .map((section) => ({
            title: section.title,
            from:
              section.source === 'plugin' && section.provider
                ? t('来自：{0}', section.provider)
                : undefined,
            rows: section.rows,
          })),
      ]
    : []

  return (
    <Modal open={!!info} onClose={onClose} title={t('显示简介')} className="file-info-modal">
      {info && (
        <div className="file-info">
          <FileIcon entry={info} size={72} />
          <h3>{info.name}</h3>
          <div className="file-info-scroll">
            <dl>
              {groups.map((group, groupIndex) => (
                <Fragment key={`${group.title}:${groupIndex}`}>
                  <div className="file-info-group">
                    <span>{group.title}</span>
                    {group.from && <small>{group.from}</small>}
                  </div>
                  {group.rows.map((row, rowIndex) => (
                    <div className="file-info-row" key={`${row.label}:${rowIndex}`}>
                      <dt>{row.label}</dt>
                      <dd>
                        <span>{row.value}</span>
                        {row.copyable && (
                          <button
                            type="button"
                            className="info-copy"
                            title={copied === `${groupIndex}:${rowIndex}` ? t('已复制') : t('复制')}
                            aria-label={t('复制')}
                            onClick={() => void copy(row.value, `${groupIndex}:${rowIndex}`)}
                          >
                            {copied === `${groupIndex}:${rowIndex}` ? (
                              <Check size={12} />
                            ) : (
                              <Copy size={12} />
                            )}
                          </button>
                        )}
                      </dd>
                    </div>
                  ))}
                </Fragment>
              ))}
            </dl>
            {loading && <p className="file-info-status">{t('正在读取媒体与插件信息…')}</p>}
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

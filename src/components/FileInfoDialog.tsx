import { ExternalLink } from 'lucide-react'
import { Fragment, useEffect, useState } from 'react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { FileIcon } from './FileIcon'
import { date, parent, kind, size } from '@/lib/files'
import { tagHex } from '@/lib/tags'
import { call } from '@/lib/api'
import type { Entry } from '@/lib/types'

interface InfoRow {
  label: string
  value: string
}
interface InfoSection {
  id: string
  source: 'core' | 'plugin'
  title: string
  pluginId?: string
  provider: string
  rows: InfoRow[]
}

export function FileInfoDialog({
  info,
  onClose,
  onError,
}: {
  info: Entry | null
  onClose: () => void
  onError: (s: string) => void
}) {
  const [sections, setSections] = useState<InfoSection[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!info) return
    let alive = true
    setSections([])
    setError('')
    setLoading(true)
    void call<InfoSection[]>('file_info_details', { path: info.path })
      .then((value) => {
        if (alive) setSections(Array.isArray(value) ? value : [])
      })
      .catch((cause) => {
        if (alive) setError(String(cause))
      })
      .finally(() => {
        if (alive) setLoading(false)
      })
    return () => {
      alive = false
    }
  }, [info])

  async function reveal() {
    if (!info) return
    try {
      await call('reveal_path', { path: info.path })
    } catch (cause) {
      onError(String(cause))
    }
  }

  const basicRows: InfoRow[] = info
    ? [
        { label: '种类', value: kind(info) },
        { label: '大小', value: info.isDir ? '文件夹' : size(info.size) },
        { label: '位置', value: parent(info.path) },
        { label: '创建时间', value: date(info.created) },
        { label: '修改时间', value: date(info.modified) },
        { label: '权限', value: info.readonly ? '只读' : '可读写' },
        { label: '符号链接', value: info.isSymlink ? '是' : '否' },
        ...(info.linkTarget ? [{ label: '链接到', value: info.linkTarget }] : []),
      ]
    : []
  // Core's metadata provider owns the basic attributes. Only use the local
  // Entry fallback for old responses or paths Core could not stat.
  const hasCoreMetadata = sections.some((section) => section.id === 'core.metadata')
  const groups: Array<{ title: string; from?: string; rows: InfoRow[] }> = info
    ? [
        ...(hasCoreMetadata ? [] : [{ title: '文件', rows: basicRows }]),
        ...sections
          .filter((section) => section.rows.length > 0)
          .map((section) => ({
            title: section.title,
            from:
              section.source === 'plugin' && section.provider
                ? `来自：${section.provider}`
                : undefined,
            rows: section.rows,
          })),
      ]
    : []

  return (
    <Modal open={!!info} onClose={onClose} title="显示简介" className="file-info-modal">
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
                      <dd>{row.value}</dd>
                    </div>
                  ))}
                </Fragment>
              ))}
            </dl>
            {loading && <p className="file-info-status">正在读取媒体与插件信息…</p>}
            {!loading && error && <p className="file-info-status">{error}</p>}
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
          <Button variant="secondary" onClick={() => void reveal()}>
            在系统文件管理器中显示
            <ExternalLink />
          </Button>
        </div>
      )}
    </Modal>
  )
}

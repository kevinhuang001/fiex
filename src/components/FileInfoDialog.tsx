import { ExternalLink } from 'lucide-react'
import { useEffect, useState } from 'react'
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
  source: 'core' | 'plugin'
  title: string
  pluginId?: string
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

  return (
    <Modal open={!!info} onClose={onClose} title="显示简介" className="file-info-modal">
      {info && (
        <div className="file-info">
          <div className="file-info-heading">
            <FileIcon entry={info} size={56} />
            <div>
              <h3>{info.name}</h3>
              <span>{kind(info)}</span>
            </div>
          </div>
          <div className="file-info-columns">
            <section className="file-info-section">
              <h4>文件</h4>
              <dl>
                {[
                  ['种类', kind(info)],
                  ['大小', info.isDir ? '文件夹' : size(info.size)],
                  ['位置', parent(info.path)],
                  ['创建时间', date(info.created)],
                  ['修改时间', date(info.modified)],
                  ['权限', info.readonly ? '只读' : '可读写'],
                  ['符号链接', info.isSymlink ? '是' : '否'],
                  ...(info.linkTarget ? [['链接到', info.linkTarget]] : []),
                ].map(([key, value]) => (
                  <div key={key}>
                    <dt>{key}</dt>
                    <dd>{value}</dd>
                  </div>
                ))}
              </dl>
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
            </section>
            <section className="file-info-section file-info-extra">
              <h4>简介</h4>
              {loading && <p className="file-info-empty">正在读取媒体与插件信息…</p>}
              {!loading && error && <p className="file-info-empty">{error}</p>}
              {!loading && !error && sections.length === 0 && (
                <p className="file-info-empty">没有额外的简介信息。</p>
              )}
              {!loading &&
                !error &&
                sections.map((section, index) => (
                  <div className="file-info-group" key={`${section.title}:${index}`}>
                    <h5>
                      {section.title}
                      {section.pluginId && <small>{section.pluginId}</small>}
                    </h5>
                    <dl>
                      {section.rows.map((row, rowIndex) => (
                        <div key={`${row.label}:${rowIndex}`}>
                          <dt>{row.label}</dt>
                          <dd>{row.value}</dd>
                        </div>
                      ))}
                    </dl>
                  </div>
                ))}
            </section>
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

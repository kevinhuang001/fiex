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

  const rows: Array<[string, string]> = info
    ? [
        ['种类', kind(info)],
        ['大小', info.isDir ? '文件夹' : size(info.size)],
        ['位置', parent(info.path)],
        ['创建时间', date(info.created)],
        ['修改时间', date(info.modified)],
        ['权限', info.readonly ? '只读' : '可读写'],
        ['符号链接', info.isSymlink ? '是' : '否'],
        ...(info.linkTarget ? ([['链接到', info.linkTarget]] as Array<[string, string]>) : []),
        ...sections.flatMap((section) =>
          section.rows.map((row) => [row.label, row.value] as [string, string]),
        ),
      ]
    : []

  return (
    <Modal open={!!info} onClose={onClose} title="显示简介" className="file-info-modal">
      {info && (
        <div className="file-info">
          <FileIcon entry={info} size={72} />
          <h3>{info.name}</h3>
          <dl>
            {rows.map(([key, value], index) => (
              <div key={`${key}:${index}`}>
                <dt>{key}</dt>
                <dd>{value}</dd>
              </div>
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
          <Button variant="secondary" onClick={() => void reveal()}>
            在系统文件管理器中显示
            <ExternalLink />
          </Button>
        </div>
      )}
    </Modal>
  )
}

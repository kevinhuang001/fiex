import { ExternalLink } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { gitState } from '@/lib/git'
import { FileIcon } from './FileIcon'
import { date, parent, kind, size } from '@/lib/files'
import { tagColor } from '@/lib/tags'
import { call } from '@/lib/api'
import type { Entry } from '@/lib/types'
export function FileInfoDialog({
  info,
  gitStatus,
  onClose,
  onError,
}: {
  info: Entry | null
  gitStatus?: string
  onClose: () => void
  onError: (s: string) => void
}) {
  async function run(_label: string, fn: () => Promise<unknown>) {
    try {
      await fn()
    } catch (e) {
      onError(String(e))
    }
  }
  return (
    <Modal open={!!info} onClose={onClose} title="显示简介">
      {info && (
        <div className="file-info">
          <FileIcon entry={info} size={72} />
          <h3>{info.name}</h3>
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
              ...(gitStatus ? [['Git', gitState(gitStatus)?.label || gitStatus]] : []),
            ].map(([k, v]) => (
              <div key={k}>
                <dt>{k}</dt>
                <dd>{v}</dd>
              </div>
            ))}
          </dl>
          {info.tags.length > 0 && (
            <div className="file-info-tags">
              {info.tags.map((tag) => (
                <span className="tag-chip" key={`${tag.name}:${tag.color}`}>
                  <i style={{ background: tagColor(tag.color) }} />
                  {tag.name}
                </span>
              ))}
            </div>
          )}
          <Button
            variant="secondary"
            disabled={info.path.startsWith('history://')}
            onClick={() => void run('正在显示…', () => call('reveal_path', { path: info.path }))}
          >
            在系统文件管理器中显示
            <ExternalLink />
          </Button>
        </div>
      )}
    </Modal>
  )
}

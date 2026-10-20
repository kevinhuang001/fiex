import { lazy, Suspense, useEffect, useState } from 'react'
import { ExternalLink, ChevronLeft, ChevronRight, AlertCircle } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { FileIcon } from './FileIcon'
import { call, asset, native } from '@/lib/api'
import { date, isImage, isText, kind, size } from '@/lib/files'
import type { Entry } from '@/lib/types'
const PdfPreview = lazy(() => import('./PdfPreview'))
export function Preview({
  entry,
  onClose,
  onNavigate,
  onOpen,
}: {
  entry: Entry | null
  onClose: () => void
  onNavigate: (delta: number) => void
  onOpen: (e: Entry) => void
}) {
  const [text, setText] = useState('')
  const [error, setError] = useState('')
  useEffect(() => {
    setText('')
    setError('')
    let active = true
    if (entry && isText(entry))
      void call<string>('read_preview', { path: entry.path })
        .then((t) => {
          if (active) setText(t)
        })
        .catch((e) => {
          if (active) setError(String(e))
        })
    return () => {
      active = false
    }
  }, [entry])
  useEffect(() => {
    if (!entry) return
    const listener = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest('input,textarea,video,audio')) return
      if (e.code === 'Space') {
        e.preventDefault()
        onClose()
      }
      if (e.key === 'ArrowLeft') onNavigate(-1)
      if (e.key === 'ArrowRight') onNavigate(1)
    }
    document.addEventListener('keydown', listener)
    return () => document.removeEventListener('keydown', listener)
  }, [entry, onClose, onNavigate])
  return (
    <Modal open={!!entry} onClose={onClose} title={entry?.name || '快速查看'} wide>
      {entry && (
        <>
          <div className="preview-body">
            {error ? (
              <div className="preview-fallback">
                <AlertCircle />
                <p>{error}</p>
                <FileIcon entry={entry} size={90} />
              </div>
            ) : entry.isDir ? (
              <div className="preview-fallback">
                <FileIcon entry={entry} size={120} />
                <h3>{entry.name}</h3>
                <p>文件夹 · {entry.path}</p>
              </div>
            ) : isText(entry) ? (
              <pre>
                {text || '正在读取…'}
                {entry.size > 256 * 1024 && '\n\n…仅预览前 256 KiB'}
              </pre>
            ) : isImage(entry) && native ? (
              <img
                src={asset(entry.path)}
                alt={entry.name}
                onError={() => setError('此图片格式无法预览，可用默认应用打开。')}
              />
            ) : entry.mime.startsWith('video/') && native ? (
              <video
                controls
                autoPlay
                src={asset(entry.path)}
                onError={() => setError('系统不支持此视频编码，可用默认应用打开。')}
              />
            ) : entry.mime.startsWith('audio/') && native ? (
              <div className="preview-fallback">
                <FileIcon entry={entry} size={100} />
                <audio
                  controls
                  autoPlay
                  src={asset(entry.path)}
                  onError={() => setError('系统不支持此音频编码，可用默认应用打开。')}
                />
              </div>
            ) : entry.extension === 'pdf' && native ? (
              <Suspense fallback={<p>正在加载 PDF…</p>}>
                <PdfPreview url={asset(entry.path)} />
              </Suspense>
            ) : (
              <div className="preview-fallback">
                <FileIcon entry={entry} size={120} />
                <h3>{entry.name}</h3>
                <p>{kind(entry)} · 使用默认应用查看</p>
              </div>
            )}
          </div>
          <footer className="preview-footer">
            <Button variant="ghost" size="icon" title="上一个" onClick={() => onNavigate(-1)}>
              <ChevronLeft />
            </Button>
            <Button variant="ghost" size="icon" title="下一个" onClick={() => onNavigate(1)}>
              <ChevronRight />
            </Button>
            <span>
              {entry.isDir ? '文件夹' : size(entry.size)} · {date(entry.modified)}
            </span>
            <Button variant="secondary" onClick={() => onOpen(entry)}>
              打开
              <ExternalLink />
            </Button>
          </footer>
        </>
      )}
    </Modal>
  )
}

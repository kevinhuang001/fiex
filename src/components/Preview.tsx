import { lazy, Suspense, useEffect, useMemo, useState } from 'react'
import { ExternalLink, ChevronLeft, ChevronRight, AlertCircle } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { FileIcon } from './FileIcon'
import { call, asset, native } from '@/lib/api'
import { date, isImage, isText, kind, size } from '@/lib/files'
import { highlightCode } from '@/lib/highlight'
import { usePluginCatalog } from '@/lib/plugins'
import { resolvePreviewer } from '@/lib/preview'
import { OPEN_PLUGIN_PANEL } from '@/components/plugins/PluginPanelHost'
import type { Entry } from '@/lib/types'
const PdfPreview = lazy(() => import('./PdfPreview'))
const dirname = (path: string) => {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return index > 0 ? path.slice(0, index) : path
}

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
  const [loading, setLoading] = useState(false)
  const [media, setMedia] = useState<{ width: number; height: number } | null>(null)
  const plugins = usePluginCatalog()
  const previewer = useMemo(
    () => (entry ? resolvePreviewer(plugins, entry.extension) : null),
    [entry, plugins],
  )
  // A plugin previewer renders in the plugin's own sandboxed panel: it gets the
  // file through the preview context instead of reading it in the webview.
  const pluginPreview = previewer?.pluginId ? previewer : null
  const source = entry && !pluginPreview ? asset(entry.path) : ''
  useEffect(() => {
    if (!entry || !pluginPreview) return
    window.dispatchEvent(
      new CustomEvent(OPEN_PLUGIN_PANEL, {
        detail: {
          pluginId: pluginPreview.pluginId,
          panelId: pluginPreview.panelId,
          title: entry.name,
          directory: dirname(entry.path),
          selection: [entry.path],
          source: 'preview',
          preview: { path: entry.path, name: entry.name, extension: entry.extension },
        },
      }),
    )
    onClose()
  }, [entry?.path, pluginPreview?.id])
  const highlighted = useMemo(
    () => (entry && isText(entry) ? highlightCode(text, entry.extension, entry.name) : null),
    [entry, text],
  )
  // Quick Look sizing: the sheet hugs the media so a photo is not surrounded by
  // an oversized empty canvas, while huge images still fit inside the window.
  const fit = useMemo(() => {
    if (!media || !media.width || !media.height) return null
    const maxWidth = Math.round(window.innerWidth * 0.92)
    const maxHeight = Math.round(window.innerHeight * 0.76)
    const scale = Math.min(1, maxWidth / media.width, maxHeight / media.height)
    return {
      width: Math.max(320, Math.round(media.width * scale)),
      height: Math.round(media.height * scale),
    }
  }, [media])
  useEffect(() => {
    setMedia(null)
  }, [entry?.path])
  useEffect(() => {
    setText('')
    setError('')
    let active = true
    setLoading(Boolean(entry && isText(entry)))
    if (entry && isText(entry))
      void call<string>('read_preview', { path: entry.path })
        .then((t) => {
          if (active) setText(t)
        })
        .catch((e) => {
          if (active) setError(String(e))
        })
        .finally(() => {
          if (active) setLoading(false)
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
    <Modal
      open={!!entry}
      onClose={onClose}
      title={entry?.name || '快速查看'}
      wide
      className={`preview-modal ${entry && isText(entry) ? 'preview-text' : entry?.extension === 'pdf' ? 'preview-pdf' : fit ? 'preview-media' : 'preview-compact'}`}
      contentStyle={fit ? { width: fit.width } : undefined}
    >
      {entry && (
        <>
          <div className="preview-body" style={fit ? { height: fit.height } : undefined}>
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
              <pre className={highlighted ? 'preview-code' : undefined}>
                {loading ? (
                  '正在读取…'
                ) : highlighted ? (
                  <code
                    className="hljs"
                    // highlight.js escapes the source before adding spans.
                    dangerouslySetInnerHTML={{ __html: highlighted }}
                  />
                ) : (
                  text || '（空文件）'
                )}
                {!loading && entry.size > 256 * 1024 && '\n\n…仅预览前 256 KiB'}
              </pre>
            ) : isImage(entry) && native ? (
              <img
                src={source}
                alt={entry.name}
                onLoad={(event) =>
                  setMedia({
                    width: event.currentTarget.naturalWidth,
                    height: event.currentTarget.naturalHeight,
                  })
                }
                onError={() => setError('此图片格式无法预览，可用默认应用打开。')}
              />
            ) : entry.mime.startsWith('video/') && native ? (
              <video
                controls
                autoPlay
                src={source}
                onLoadedMetadata={(event) =>
                  setMedia({
                    width: event.currentTarget.videoWidth,
                    height: event.currentTarget.videoHeight,
                  })
                }
                onError={() => setError('系统不支持此视频编码，可用默认应用打开。')}
              />
            ) : entry.mime.startsWith('audio/') && native ? (
              <div className="preview-fallback">
                <FileIcon entry={entry} size={100} />
                <audio
                  controls
                  autoPlay
                  src={source}
                  onError={() => setError('系统不支持此音频编码，可用默认应用打开。')}
                />
              </div>
            ) : entry.extension === 'pdf' && native ? (
              <Suspense fallback={<p>正在加载 PDF…</p>}>
                <PdfPreview url={source} />
              </Suspense>
            ) : (
              <div className="preview-fallback">
                <FileIcon entry={entry} size={120} />
                <h3>{entry.name}</h3>
                <p>{kind(entry)}</p>
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

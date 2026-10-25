import { lazy, Suspense, useEffect, useMemo, useRef, useState } from 'react'
import {
  ExternalLink,
  ChevronLeft,
  ChevronRight,
  AlertCircle,
  Check,
  ChevronUp,
  Monitor,
  Puzzle,
} from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { FileIcon } from './FileIcon'
import { call, asset, native } from '@/lib/api'
import { date, isImage, isText, kind, size } from '@/lib/files'
import { highlightCode } from '@/lib/highlight'
import { usePluginCatalog } from '@/lib/plugins'
import {
  BUILTIN_PREVIEWERS,
  pluginPreviewers,
  previewDisabled,
  resolvePreviewer,
} from '@/lib/preview'
import { PluginPreviewFrame } from '@/components/plugins/PluginPreviewFrame'
import type { Entry } from '@/lib/types'
import type { Previewer } from '@/lib/preview'
const PdfPreview = lazy(() => import('./PdfPreview'))

function builtinCandidates(entry: Entry): Previewer[] {
  const values: Previewer[] = []
  if (isText(entry)) values.push(BUILTIN_PREVIEWERS[0])
  if (isImage(entry)) values.push(BUILTIN_PREVIEWERS[1])
  if (entry.mime.startsWith('video/') || entry.mime.startsWith('audio/'))
    values.push(BUILTIN_PREVIEWERS[2])
  if (entry.extension === 'pdf') values.push(BUILTIN_PREVIEWERS[3])
  values.push(BUILTIN_PREVIEWERS[4])
  return values
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
  const [picked, setPicked] = useState<string | null>(null)
  const [menuOpen, setMenuOpen] = useState(false)
  const sourceRef = useRef<HTMLDivElement>(null)
  const plugins = usePluginCatalog()
  const pluginCandidates = useMemo(() => {
    if (!entry) return []
    const disabled = new Set(previewDisabled())
    return pluginPreviewers(plugins)
      .filter(
        (previewer) =>
          !disabled.has(previewer.id) &&
          previewer.extensions.includes(entry.extension.toLowerCase()),
      )
      .sort((a, b) => b.priority - a.priority || a.id.localeCompare(b.id))
  }, [entry, plugins])
  const builtins = useMemo(() => (entry ? builtinCandidates(entry) : []), [entry])
  const candidates = useMemo(() => [...pluginCandidates, ...builtins], [pluginCandidates, builtins])
  const previewer = useMemo(() => {
    if (!entry) return null
    const chosen = candidates.find((candidate) => candidate.id === picked)
    if (chosen) return chosen
    const resolved = resolvePreviewer(plugins, entry.extension)
    return resolved.pluginId ? resolved : builtins[0] || resolved
  }, [entry, candidates, picked, plugins, builtins])
  // A plugin previewer renders in the plugin's own sandboxed panel: it gets the
  // file through the preview context instead of reading it in the webview.
  const pluginPreview = previewer?.pluginId ? previewer : null
  const [materialized, setMaterialized] = useState<{ path: string; local: string } | null>(null)
  useEffect(() => {
    let active = true
    if (
      entry &&
      !entry.isDir &&
      native &&
      !pluginPreview &&
      /^fiex:/i.test(entry.path) &&
      !isText(entry)
    ) {
      void call<string>('materialize_file', { path: entry.path })
        .then((local) => {
          if (active) setMaterialized({ path: entry.path, local })
        })
        .catch((error) => {
          if (active) setError(String(error))
        })
    }
    return () => {
      active = false
    }
  }, [entry?.path, Boolean(pluginPreview)])
  const localPath = entry && materialized?.path === entry.path ? materialized.local : entry?.path
  const source = localPath && !pluginPreview ? asset(localPath) : ''
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
    setPicked(null)
    setMenuOpen(false)
    setError('')
  }, [entry?.path])
  useEffect(() => {
    if (!menuOpen) return
    const close = (event: MouseEvent) => {
      if (!sourceRef.current?.contains(event.target as Node)) setMenuOpen(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [menuOpen])
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
      className={`preview-modal ${pluginPreview ? 'preview-plugin' : entry && isText(entry) ? 'preview-text' : entry?.extension === 'pdf' ? 'preview-pdf' : fit ? 'preview-media' : 'preview-compact'}`}
      contentStyle={
        pluginPreview ? { width: 'min(92vw, 980px)' } : fit ? { width: fit.width } : undefined
      }
    >
      {entry && (
        <>
          <div className="preview-body" style={fit ? { height: fit.height } : undefined}>
            {native &&
            entry.path.startsWith('fiex:') &&
            !entry.isDir &&
            (!isText(entry) || pluginPreview) &&
            materialized?.path !== entry.path &&
            !error ? (
              <p>正在读取…</p>
            ) : pluginPreview && entry && !error ? (
              <PluginPreviewFrame
                key={localPath}
                pluginId={pluginPreview.pluginId || ''}
                contributionId={pluginPreview.panelId || ''}
                path={localPath || entry.path}
                name={entry.name}
                extension={entry.extension}
              />
            ) : error ? (
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
            <div className="preview-source" ref={sourceRef}>
              <button
                type="button"
                className="preview-source-button"
                aria-haspopup="listbox"
                aria-expanded={menuOpen}
                title="切换预览方式"
                onClick={() => setMenuOpen((value) => !value)}
              >
                {previewer?.pluginId ? <Puzzle size={13} /> : <Monitor size={13} />}
                <span>{previewer?.provider || 'Fiex'}</span>
                <small>{previewer?.title}</small>
                <ChevronUp
                  size={12}
                  style={{ transform: menuOpen ? undefined : 'rotate(180deg)' }}
                />
              </button>
              {menuOpen && (
                <div className="preview-source-menu" role="listbox" aria-label="预览方式">
                  {pluginCandidates.length > 0 && <div className="preview-source-label">插件</div>}
                  {pluginCandidates.map((candidate) => (
                    <button
                      type="button"
                      role="option"
                      aria-selected={previewer?.id === candidate.id}
                      className={
                        'preview-source-item' + (previewer?.id === candidate.id ? ' active' : '')
                      }
                      key={candidate.id}
                      onClick={() => {
                        setPicked(candidate.id)
                        setMenuOpen(false)
                      }}
                    >
                      <Puzzle size={14} />
                      <span className="preview-source-copy">
                        <span className="preview-source-title">{candidate.title}</span>
                        <span className="preview-source-provider">{candidate.provider}</span>
                      </span>
                      {previewer?.id === candidate.id && <Check size={14} />}
                    </button>
                  ))}
                  <div className="preview-source-label">内置</div>
                  {builtins.map((candidate) => (
                    <button
                      type="button"
                      role="option"
                      aria-selected={previewer?.id === candidate.id}
                      className={
                        'preview-source-item' + (previewer?.id === candidate.id ? ' active' : '')
                      }
                      key={candidate.id}
                      onClick={() => {
                        setPicked(candidate.id)
                        setMenuOpen(false)
                      }}
                    >
                      <Monitor size={14} />
                      <span className="preview-source-copy">
                        <span className="preview-source-title">{candidate.title}</span>
                        <span className="preview-source-provider">Fiex 内置</span>
                      </span>
                      {previewer?.id === candidate.id && <Check size={14} />}
                    </button>
                  ))}
                </div>
              )}
            </div>
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

import { lazy, Suspense, useEffect, useMemo, useRef, useState } from 'react'
import {
  AlertCircle,
  Check,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  ExternalLink,
  Monitor,
  Puzzle,
} from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { FileIcon } from './FileIcon'
import { call, asset, native } from '@/lib/api'
import { date, kind, size } from '@/lib/files'
import { highlightCode } from '@/lib/highlight'
import { orderCandidates, previewDisabled, type PreviewCandidate } from '@/lib/preview'
import { PluginPreviewFrame } from '@/components/plugins/PluginPreviewFrame'
import type { Entry } from '@/lib/types'
const PdfPreview = lazy(() => import('./PdfPreview'))

interface ResolvedPreview {
  providerId: string
  provider: string
  priority: number
  view: {
    kind: 'html' | 'text' | 'image' | 'media' | 'pdf' | 'fallback'
    html?: string
    text?: string
    truncated?: boolean
    path?: string
    name?: string
    mime?: string
    media?: 'video' | 'audio'
    message?: string
    sessionId?: string
    pluginId?: string
    contributionId?: string
    context?: unknown
  }
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
  const [candidates, setCandidates] = useState<PreviewCandidate[]>([])
  const [providerId, setProviderId] = useState<string | null>(null)
  const [resolved, setResolved] = useState<ResolvedPreview | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [media, setMedia] = useState<{ width: number; height: number } | null>(null)
  const [menuOpen, setMenuOpen] = useState(false)
  const sourceRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    let active = true
    setCandidates([])
    setProviderId(null)
    setResolved(null)
    setError('')
    setMedia(null)
    setMenuOpen(false)
    if (!entry) return
    setLoading(true)
    void call<PreviewCandidate[]>('preview_candidates', {
      extension: entry.extension,
      mime: entry.mime,
      isDir: entry.isDir,
    })
      .then((values) => {
        if (!active) return
        const disabled = new Set(previewDisabled())
        const allowed = orderCandidates(
          (Array.isArray(values) ? values : []).filter((value) => !disabled.has(value.id)),
        )
        setCandidates(allowed)
        setProviderId(allowed[0]?.id ?? null)
        if (!allowed.length) setError('没有可用的预览方式')
      })
      .catch((cause) => {
        if (active) setError(String(cause))
      })
      .finally(() => {
        if (active) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [entry?.path])

  useEffect(() => {
    if (!entry || !providerId) return
    let active = true
    setLoading(true)
    setError('')
    setResolved(null)
    setMedia(null)
    void call<ResolvedPreview>('preview_open', {
      path: entry.path,
      providerId,
      providerOrder: candidates.map((candidate) => candidate.id),
    })
      .then((value) => {
        if (active) setResolved(value)
      })
      .catch((cause) => {
        if (active) setError(String(cause))
      })
      .finally(() => {
        if (active) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [entry?.path, providerId])

  useEffect(() => {
    if (!menuOpen) return
    const close = (event: MouseEvent) => {
      if (!sourceRef.current?.contains(event.target as Node)) setMenuOpen(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [menuOpen])

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

  const view = resolved?.view
  const fileSource = view?.path ? (native ? asset(view.path) : view.path) : ''
  const highlighted = useMemo(
    () =>
      entry && view?.kind === 'text'
        ? highlightCode(view.text || '', entry.extension, entry.name)
        : null,
    [entry, view?.kind, view?.text],
  )
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
  const current = candidates.find((candidate) => candidate.id === providerId)
  const provider = resolved?.provider || current?.provider || 'Fiex'
  const fromPlugin = providerId?.startsWith('plugin:')
  const providerLabel = fromPlugin ? provider : 'Fiex 内置'

  return (
    <Modal
      open={!!entry}
      onClose={onClose}
      title={entry?.name || '快速查看'}
      wide
      className={`preview-modal ${
        view?.kind === 'html'
          ? 'preview-plugin'
          : view?.kind === 'text'
            ? 'preview-text'
            : view?.kind === 'pdf'
              ? 'preview-pdf'
              : fit
                ? 'preview-media'
                : 'preview-compact'
      }`}
      contentStyle={
        view?.kind === 'html'
          ? { width: 'min(92vw, 980px)' }
          : fit
            ? { width: fit.width }
            : undefined
      }
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
            ) : loading && !resolved ? (
              <p>正在读取…</p>
            ) : view?.kind === 'html' && view.html ? (
              <PluginPreviewFrame
                key={`${entry.path}:${providerId}`}
                pluginId={view.pluginId || ''}
                contributionId={view.contributionId || ''}
                html={view.html}
                context={view.context}
                sessionId={view.sessionId || ''}
                name={entry.name}
                extension={entry.extension}
              />
            ) : view?.kind === 'text' ? (
              <pre className={highlighted ? 'preview-code' : undefined}>
                {highlighted ? (
                  <code className="hljs" dangerouslySetInnerHTML={{ __html: highlighted }} />
                ) : (
                  view.text || '（空文件）'
                )}
                {view.truncated && '\n\n…仅预览前 256 KiB'}
              </pre>
            ) : view?.kind === 'image' ? (
              <img
                src={fileSource}
                alt={entry.name}
                onLoad={(event) =>
                  setMedia({
                    width: event.currentTarget.naturalWidth,
                    height: event.currentTarget.naturalHeight,
                  })
                }
                onError={() => setError('此图片格式无法预览，可用默认应用打开。')}
              />
            ) : view?.kind === 'media' && view.media === 'video' ? (
              <video
                controls
                autoPlay
                src={fileSource}
                onLoadedMetadata={(event) =>
                  setMedia({
                    width: event.currentTarget.videoWidth,
                    height: event.currentTarget.videoHeight,
                  })
                }
                onError={() => setError('系统不支持此视频编码，可用默认应用打开。')}
              />
            ) : view?.kind === 'media' ? (
              <div className="preview-fallback">
                <FileIcon entry={entry} size={100} />
                <audio
                  controls
                  autoPlay
                  src={fileSource}
                  onError={() => setError('系统不支持此音频编码，可用默认应用打开。')}
                />
              </div>
            ) : view?.kind === 'pdf' && fileSource ? (
              <Suspense fallback={<p>正在加载 PDF…</p>}>
                <PdfPreview url={fileSource} />
              </Suspense>
            ) : (
              <div className="preview-fallback">
                <FileIcon entry={entry} size={120} />
                <h3>{view?.name || entry.name}</h3>
                <p>{view?.message || kind(entry)}</p>
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
            {candidates.length > 0 && (
              <div className="preview-source" ref={sourceRef}>
                <button
                  type="button"
                  className="preview-source-button"
                  aria-haspopup="listbox"
                  aria-expanded={menuOpen}
                  title="切换预览方式"
                  onClick={() => setMenuOpen((value) => !value)}
                >
                  {fromPlugin ? <Puzzle size={13} /> : <Monitor size={13} />}
                  <span>{providerLabel}</span>
                  <ChevronUp
                    size={12}
                    style={{ transform: menuOpen ? undefined : 'rotate(180deg)' }}
                  />
                </button>
                {menuOpen && (
                  <div className="preview-source-menu" role="listbox" aria-label="预览方式">
                    {candidates.map((candidate) => (
                      <button
                        type="button"
                        role="option"
                        aria-selected={providerId === candidate.id}
                        className={
                          'preview-source-item' + (providerId === candidate.id ? ' active' : '')
                        }
                        key={candidate.id}
                        onClick={() => {
                          setProviderId(candidate.id)
                          setMenuOpen(false)
                        }}
                      >
                        {candidate.id.startsWith('plugin:') ? (
                          <Puzzle size={14} />
                        ) : (
                          <Monitor size={14} />
                        )}
                        <span className="preview-source-copy">
                          <span className="preview-source-title">
                            {candidate.id.startsWith('plugin:') ? candidate.provider : 'Fiex 内置'}
                          </span>
                        </span>
                        {providerId === candidate.id && <Check size={14} />}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
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

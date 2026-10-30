import type { ResolvedPreview, PluginLoadingPage } from './preview/types'
import { t, getLocale } from '@/i18n'
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
import {
  orderCandidates,
  pluginPreviewId,
  previewDisabled,
  type PreviewCandidate,
} from '@/lib/preview'
import { PluginPreviewFrame } from '@/components/plugins/PluginPreviewFrame'
import type { Entry } from '@/lib/types'
import { MemoryCache } from '@/lib/cache'
const PdfPreview = lazy(() => import('./PdfPreview'))
// Candidate discovery depends on the installed/enabled plugins; keep it brief.
const candidateCache = new MemoryCache<PreviewCandidate[]>(64, 3_000)
/** Package-local loading document (Core-owned asset path or inline HTML). */
type LoadingDocument = { html?: string; src?: string }
// A loading document belongs to the plugin package, not to the file being
// previewed. Cache it so provider switches and repeated opens can paint it
// immediately instead of waiting for another IPC round-trip.
const loadingDocumentCache = new MemoryCache<LoadingDocument | null>(16, 5 * 60_000)
function loadingDocument(pluginId: string): Promise<LoadingDocument | null> {
  return loadingDocumentCache.get(pluginId, async () => {
    if (native) {
      const path = await call<string | null>('preview_loading_asset', { pluginId })
      return path ? { src: asset(path) } : null
    }
    const html = await call<string | null>('preview_loading', { pluginId })
    return html ? { html } : null
  })
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
  const [providerAttempt, setProviderAttempt] = useState(0)
  const [resolved, setResolved] = useState<ResolvedPreview | null>(null)
  const [loading, setLoading] = useState(false)
  const [loadingPage, setLoadingPage] = useState<PluginLoadingPage | null>(null)
  const [previewReady, setPreviewReady] = useState(false)
  const [previewAsset, setPreviewAsset] = useState('')
  const [readyToShow, setReadyToShow] = useState(false)
  const [error, setError] = useState('')
  const [media, setMedia] = useState<{ width: number; height: number } | null>(null)
  const [menuOpen, setMenuOpen] = useState(false)
  const sourceRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    let active = true
    setCandidates([])
    setProviderId(null)
    setResolved(null)
    setLoadingPage(null)
    setPreviewReady(false)
    setPreviewAsset('')
    setReadyToShow(false)
    setError('')
    setMedia(null)
    setMenuOpen(false)
    if (!entry) return
    setLoading(true)
    void candidateCache
      .get(
        JSON.stringify([
          entry.path,
          entry.modified,
          entry.allowedOps,
          entry.extension,
          entry.mime,
          entry.isDir,
        ]),
        () =>
          call<PreviewCandidate[]>('preview_candidates', {
            path: entry.path,
            extension: entry.extension,
            mime: entry.mime,
            isDir: entry.isDir,
          }),
      )
      .then((values) => {
        if (!active) return
        const disabled = new Set(previewDisabled())
        const allowed = orderCandidates(
          (Array.isArray(values) ? values : []).filter((value) => !disabled.has(value.id)),
        )
        setCandidates(allowed)
        setProviderId(allowed[0]?.id ?? null)
        if (!allowed.length) setError(t('没有可用的预览方式'))
        setReadyToShow(true)
        // Warm every declared loading document as soon as the providers are
        // known, before the first `preview.open` leaves the UI. The document is
        // a local package file, so this is cheap and gives the selected
        // provider a ready-to-paint loading page on the very next render.
        for (const candidate of allowed) {
          const pluginId = pluginPreviewId(candidate.id)
          if (pluginId && candidate.loading) void loadingDocument(pluginId).catch(() => {})
        }
      })
      .catch((cause) => {
        if (active) {
          setError(String(cause))
          setReadyToShow(true)
        }
      })
    return () => {
      active = false
    }
  }, [entry?.path])

  useEffect(() => {
    if (!entry || !providerId) return
    let active = true
    const pluginId = pluginPreviewId(providerId)
    const declaresLoading = Boolean(
      pluginId && candidates.find((candidate) => candidate.id === providerId)?.loading,
    )
    setLoading(true)
    setError('')
    setResolved(null)
    setMedia(null)
    setLoadingPage(null)
    setPreviewReady(!declaresLoading)
    setPreviewAsset('')
    // Resolve the declared loading document first and keep the request running
    // independently of `preview.open`: a warm plugin answers immediately, but
    // its real document may take minutes to convert. The document is read from
    // the package by Core (no plugin process involved), so it can paint long
    // before the provider result arrives.
    if (pluginId && declaresLoading) {
      void loadingDocument(pluginId)
        .then((document) => {
          if (!active) return
          if (document) {
            setLoadingPage({ pluginId, sessionId: crypto.randomUUID(), ...document })
          } else {
            // No document after all: don't block the real preview behind a
            // placeholder that will never be replaced.
            setPreviewReady(true)
          }
        })
        .catch(() => {
          // A broken loading document must not block the actual preview.
          if (active) {
            setPreviewReady(true)
          }
        })
    }
    const result = call<ResolvedPreview>('preview_open', {
      path: entry.path,
      providerId,
      providerOrder: candidates.map((candidate) => candidate.id),
    }).then(
      (value) => ({ value }),
      (error) => ({ error }),
    )
    const open = async () => {
      const outcome = await result
      if ('error' in outcome) throw outcome.error
      const value = outcome.value
      if (!active) return
      // Packaged builds must load plugin HTML from the asset origin: a srcDoc
      // document inherits the app shell CSP and blocks the plugin's inline
      // bundle. Browser/demo builds keep the inline fallback.
      if (native && value.view?.kind === 'html' && value.view.pluginId && value.view.entry) {
        try {
          const path = await call<string>('plugin_view_asset', {
            pluginId: value.view.pluginId,
            entry: value.view.entry,
          })
          if (active) setPreviewAsset(asset(path))
        } catch {
          // Fall back to srcDoc below; the bridge will report a clear timeout.
        }
      }
      if (!active) return
      setResolved(value)
      // Non-HTML views have no plugin shell to cover. An HTML view keeps the
      // loading document (or its placeholder) visible until the preview calls
      // `preview.ready`, regardless of how long the conversion takes.
      if (value.view?.kind !== 'html') {
        setLoadingPage(null)
        setPreviewReady(true)
      } else if (!declaresLoading) {
        setPreviewReady(true)
      }
    }
    void open()
      .catch((cause) => {
        if (!active) return
        setError(String(cause))
        setLoadingPage(null)
      })
      .finally(() => {
        if (active) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [entry?.path, providerId, providerAttempt])

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
    [entry, view?.kind, view?.text, getLocale()],
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
  }, [media, getLocale()])
  const current = candidates.find(
    (candidate) => candidate.id === (resolved?.providerId || providerId),
  )
  const provider = resolved?.provider || current?.provider || 'Fiex'
  const fromPlugin = pluginPreviewId(resolved?.providerId || providerId) !== null
  const providerLabel = fromPlugin ? provider : t('Fiex 内置')
  const activePluginId = pluginPreviewId(providerId)
  const declaresLoading = Boolean(
    activePluginId &&
    candidates.some((candidate) => candidate.id === providerId && candidate.loading),
  )
  // The loading sheet owns the preview area from the moment the provider is
  // selected until the plugin reports its first render. It must survive a slow
  // conversion (large Office files) as well as a slow `preview.open`.
  const loadingPreview = Boolean(!error && declaresLoading && !previewReady)
  // Placeholder shown only while the package-local loading document is being
  // resolved (normally a single IPC round-trip behind the prefetch).
  const waitingForLoadingDocument = Boolean(loadingPreview && !loadingPage)
  const pluginSheet = view?.kind === 'html' || loadingPreview
  const loadingPlaceholder = <div className="preview-plugin-state">{t('正在准备预览…')}</div>

  return (
    <Modal
      open={!!entry && readyToShow}
      onClose={onClose}
      title={entry?.name || t('快速查看')}
      wide
      className={`preview-modal ${
        pluginSheet
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
        pluginSheet ? { width: 'min(92vw, 980px)' } : fit ? { width: fit.width } : undefined
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
            ) : loadingPage && !resolved ? (
              <PluginPreviewFrame
                key={`loading:${entry.path}:${providerId}:${loadingPage.sessionId}`}
                pluginId={loadingPage.pluginId}
                contributionId={loadingPage.pluginId}
                html={loadingPage.html}
                src={loadingPage.src}
                sessionId={loadingPage.sessionId}
                name={entry.name}
                extension={entry.extension}
                loading
              />
            ) : view?.kind === 'html' && view.html ? (
              waitingForLoadingDocument ? (
                loadingPlaceholder
              ) : (
                <>
                  <PluginPreviewFrame
                    key={`${entry.path}:${providerId}`}
                    pluginId={view.pluginId || ''}
                    contributionId={view.contributionId || ''}
                    html={previewAsset ? undefined : view.html}
                    src={previewAsset || undefined}
                    context={view.context}
                    sessionId={view.sessionId || ''}
                    timeoutMs={view.timeoutMs}
                    name={entry.name}
                    extension={entry.extension}
                    onPreviewReady={() => setPreviewReady(true)}
                  />
                  {loadingPage && !previewReady && (
                    <div className="preview-plugin-loading">
                      <PluginPreviewFrame
                        key={`overlay:${entry.path}:${providerId}:${loadingPage.sessionId}`}
                        pluginId={loadingPage.pluginId}
                        contributionId={loadingPage.pluginId}
                        html={loadingPage.html}
                        src={loadingPage.src}
                        sessionId={loadingPage.sessionId}
                        name={entry.name}
                        extension={entry.extension}
                        loading
                      />
                    </div>
                  )}
                </>
              )
            ) : waitingForLoadingDocument ? (
              loadingPlaceholder
            ) : loading && !resolved ? (
              <p>{t('正在读取…')}</p>
            ) : view?.kind === 'text' ? (
              <pre className={highlighted ? 'preview-code' : undefined}>
                {highlighted ? (
                  <code className="hljs" dangerouslySetInnerHTML={{ __html: highlighted }} />
                ) : (
                  view.text || t('（空文件）')
                )}
                {view.truncated && t('\n\n…仅预览前 256 KiB')}
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
                onError={() => setError(t('此图片格式无法预览，可用默认应用打开。'))}
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
                onError={() => setError(t('系统不支持此视频编码，可用默认应用打开。'))}
              />
            ) : view?.kind === 'media' ? (
              <div className="preview-fallback">
                <FileIcon entry={entry} size={100} />
                <audio
                  controls
                  autoPlay
                  src={fileSource}
                  onError={() => setError(t('系统不支持此音频编码，可用默认应用打开。'))}
                />
              </div>
            ) : view?.kind === 'pdf' && fileSource ? (
              <Suspense fallback={<p>{t('正在加载 PDF…')}</p>}>
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
            <Button variant="ghost" size="icon" title={t('上一个')} onClick={() => onNavigate(-1)}>
              <ChevronLeft />
            </Button>
            <Button variant="ghost" size="icon" title={t('下一个')} onClick={() => onNavigate(1)}>
              <ChevronRight />
            </Button>
            <span>
              {entry.isDir ? t('文件夹') : size(entry.size)} · {date(entry.modified)}
            </span>
            {candidates.length > 0 && (
              <div className="preview-source" ref={sourceRef}>
                <button
                  type="button"
                  className="preview-source-button"
                  aria-haspopup="listbox"
                  aria-expanded={menuOpen}
                  title={t('切换预览方式')}
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
                  <div className="preview-source-menu" role="listbox" aria-label={t('预览方式')}>
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
                          setProviderAttempt((value) => value + 1)
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
                            {candidate.id.startsWith('plugin:')
                              ? candidate.provider
                              : t('Fiex 内置')}
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
              {t('打开')}
              <ExternalLink />
            </Button>
          </footer>
        </>
      )}
    </Modal>
  )
}

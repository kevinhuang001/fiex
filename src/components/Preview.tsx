import type { ResolvedPreview, PluginLoadingPage } from './preview/types'
import { t } from '@/i18n'
import { useEffect, useRef, useState } from 'react'
import { call, asset, native } from '@/lib/api'
import { reportError, reportFilesystemPermissionError } from '@/lib/errorCenter'
import {
  orderCandidates,
  pluginPreviewId,
  previewDisabled,
  type PreviewCandidate,
} from '@/lib/preview'
import type { Entry } from '@/lib/types'
import { MemoryCache } from '@/lib/cache'
import { PreviewDialog } from './preview/PreviewDialog'
// Candidate discovery depends on the installed/enabled plugins; keep it brief.
const candidateCache = new MemoryCache<PreviewCandidate[]>(64, 3_000)
/** Package-local loading document served from the Core-owned asset path. */
type LoadingDocument = { src: string }
// A loading document belongs to the plugin package, not to the file being
// previewed. Cache it so provider switches and repeated opens can paint it
// immediately instead of waiting for another IPC round-trip.
const loadingDocumentCache = new MemoryCache<LoadingDocument | null>(16, 5 * 60_000)
function loadingDocument(pluginId: string): Promise<LoadingDocument | null> {
  return loadingDocumentCache.get(pluginId, async () => {
    const path = await call('preview_loading_asset', { pluginId })
    return path ? { src: asset(path) } : null
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
  const allowedOpsKey = entry?.allowedOps?.join('\0') ?? ''
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
          entry.size,
          allowedOpsKey,
          entry.extension,
          entry.mime,
          entry.isDir,
        ]),
        () =>
          call('preview_candidates', {
            path: entry.path,
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
        // Only the first selected provider's package page is needed for this
        // open. Preloading every candidate causes unnecessary disk reads.
        const selected = allowed[0]
        const pluginId = selected && pluginPreviewId(selected.id)
        if (pluginId && selected.loading) void loadingDocument(pluginId).catch(() => {})
      })
      .catch((cause) => {
        if (active) {
          setError(String(cause))
          if (!reportFilesystemPermissionError(cause, `preview\n${entry.path}`)) {
            reportError(cause, 'preview:candidates')
          }
          setReadyToShow(true)
        }
      })
    return () => {
      active = false
    }
  }, [
    entry?.path,
    entry?.modified,
    entry?.size,
    entry?.extension,
    entry?.mime,
    entry?.isDir,
    allowedOpsKey,
  ])

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
    const result = call('preview_open', {
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
      // Load packaged plugin HTML from its asset origin: a srcDoc document
      // inherits the app shell CSP and blocks the plugin's inline bundle.
      if (native && value.view?.kind === 'html' && value.view.pluginId && value.view.entry) {
        try {
          const path = await call('plugin_view_asset', {
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
        if (!reportFilesystemPermissionError(cause, `preview\n${entry.path}`)) {
          reportError(cause, 'preview:open')
        }
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

  return (
    <PreviewDialog
      entry={entry}
      onClose={onClose}
      onNavigate={onNavigate}
      onOpen={onOpen}
      sourceRef={sourceRef}
      state={{
        candidates,
        providerId,
        resolved,
        loading,
        loadingPage,
        previewReady,
        previewAsset,
        readyToShow,
        error,
        media,
        menuOpen,
      }}
      actions={{
        setPreviewReady,
        setMedia,
        setError,
        setProviderId,
        setProviderAttempt,
        setMenuOpen,
      }}
    />
  )
}

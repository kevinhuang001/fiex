import { useEffect, useRef, useState } from 'react'
import type {
  PreviewSessionActions,
  PreviewSessionInput,
  PreviewSessionResult,
  PreviewSessionServices,
} from '../types'
import { orderCandidates, pluginPreviewId } from '../model'

/** Owns provider discovery, preview opening, and plugin loading state. */
export function usePreviewSession(
  { entry, onClose, onNavigate }: PreviewSessionInput,
  services: PreviewSessionServices,
): PreviewSessionResult {
  const entryPath = entry?.path
  const entryModified = entry?.modified
  const entrySize = entry?.size
  const entryExtension = entry?.extension
  const entryMime = entry?.mime
  const entryIsDir = entry?.isDir
  const allowedOpsKey = entry?.allowedOps?.join('\0') ?? ''
  const [candidates, setCandidates] = useState<PreviewSessionResult['state']['candidates']>([])
  const [providerId, setProviderId] = useState<string | null>(null)
  const [providerAttempt, setProviderAttempt] = useState(0)
  const [resolved, setResolved] = useState<PreviewSessionResult['state']['resolved']>(null)
  const [loading, setLoading] = useState(false)
  const [loadingPage, setLoadingPage] = useState<PreviewSessionResult['state']['loadingPage']>(null)
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
    setLoading(Boolean(entryPath))
    if (!entryPath) return

    const cacheKey = JSON.stringify([
      entryPath,
      entryModified,
      entrySize,
      allowedOpsKey,
      entryExtension,
      entryMime,
      entryIsDir,
    ])
    void services
      .cachedCandidates(cacheKey, () => services.findCandidates(entryPath))
      .then((values) => {
        if (!active) return
        const preferences = services.readPreferences()
        const disabled = new Set(preferences.disabled)
        const allowed = orderCandidates(
          values.filter((value) => !disabled.has(value.id)),
          preferences.order,
        )
        setCandidates(allowed)
        setProviderId(allowed[0]?.id ?? null)
        setLoading(allowed.length > 0)
        if (!allowed.length) setError(services.noAvailablePreviewMessage())
        setReadyToShow(true)
        // Only load the first selected provider's page for this open.
        const selected = allowed[0]
        const pluginId = selected && pluginPreviewId(selected.id)
        if (pluginId && selected.loading) void services.loadingDocument(pluginId).catch(() => {})
      })
      .catch((cause) => {
        if (!active) return
        setError(String(cause))
        if (!services.reportFilesystemPermissionError(cause, `preview\n${entryPath}`)) {
          services.reportError(cause, 'preview:candidates')
        }
        setLoading(false)
        setReadyToShow(true)
      })
    return () => {
      active = false
    }
  }, [
    entryPath,
    entryModified,
    entrySize,
    entryExtension,
    entryMime,
    entryIsDir,
    allowedOpsKey,
    services,
  ])

  useEffect(() => {
    if (!entryPath || !providerId) return
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

    // Keep this request independent of preview.open so slow conversions can
    // display their package loading page immediately.
    if (pluginId && declaresLoading) {
      void services
        .loadingDocument(pluginId)
        .then((document) => {
          if (!active) return
          if (document) {
            setLoadingPage({ pluginId, sessionId: services.newSessionId(), ...document })
          } else {
            setPreviewReady(true)
          }
        })
        .catch(() => {
          if (active) setPreviewReady(true)
        })
    }

    const result = services
      .openPreview(
        entryPath,
        providerId,
        candidates.map((candidate) => candidate.id),
      )
      .then(
        (value) => ({ value }),
        (error) => ({ error }),
      )
    const open = async () => {
      const outcome = await result
      if ('error' in outcome) throw outcome.error
      const value = outcome.value
      if (!active) return
      if (
        services.isNative &&
        value.view?.kind === 'html' &&
        value.view.pluginId &&
        value.view.entry
      ) {
        try {
          const path = await services.pluginViewAsset(value.view.pluginId, value.view.entry)
          if (active) setPreviewAsset(services.toAssetUrl(path))
        } catch {
          // Fall back to srcDoc; the plugin bridge reports a clear timeout.
        }
      }
      if (!active) return
      setResolved(value)
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
        if (!services.reportFilesystemPermissionError(cause, `preview\n${entryPath}`)) {
          services.reportError(cause, 'preview:open')
        }
        setLoadingPage(null)
      })
      .finally(() => {
        if (active) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [entryPath, providerId, providerAttempt, candidates, services])

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
    const listener = (event: KeyboardEvent) => {
      if ((event.target as HTMLElement).closest('input,textarea,video,audio')) return
      if (event.code === 'Space') {
        event.preventDefault()
        onClose()
      }
      if (event.key === 'ArrowLeft') onNavigate(-1)
      if (event.key === 'ArrowRight') onNavigate(1)
    }
    document.addEventListener('keydown', listener)
    return () => document.removeEventListener('keydown', listener)
  }, [entry, onClose, onNavigate])

  const state: PreviewSessionResult['state'] = {
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
  }
  const actions: PreviewSessionActions = {
    setPreviewReady,
    setMedia,
    setError,
    setProviderId,
    setProviderAttempt,
    setMenuOpen,
  }
  return { sourceRef, state, actions }
}

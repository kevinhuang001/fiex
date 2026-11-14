import type { Dispatch, RefObject, SetStateAction } from 'react'
import { useMemo } from 'react'
import { t, getLocale } from '@/i18n'
import { Modal } from '../ui/Modal'
import { asset, native } from '@/lib/api'
import { highlightCode } from '@/lib/highlight'
import { pluginPreviewId, type PreviewCandidate } from '@/lib/preview'
import type { Entry } from '@/lib/types'
import type { PluginLoadingPage, ResolvedPreview } from './types'
import { PreviewContent } from './PreviewContent'
import { PreviewFooter } from './PreviewFooter'
import { fitPreviewMedia, previewDialogClass } from './layout'

export type PreviewDialogState = {
  candidates: PreviewCandidate[]
  providerId: string | null
  resolved: ResolvedPreview | null
  loading: boolean
  loadingPage: PluginLoadingPage | null
  previewReady: boolean
  previewAsset: string
  readyToShow: boolean
  error: string
  media: { width: number; height: number } | null
  menuOpen: boolean
}

export type PreviewDialogActions = {
  setPreviewReady: Dispatch<SetStateAction<boolean>>
  setMedia: Dispatch<SetStateAction<{ width: number; height: number } | null>>
  setError: Dispatch<SetStateAction<string>>
  setProviderId: Dispatch<SetStateAction<string | null>>
  setProviderAttempt: Dispatch<SetStateAction<number>>
  setMenuOpen: Dispatch<SetStateAction<boolean>>
}

export function PreviewDialog({
  entry,
  onClose,
  onNavigate,
  onOpen,
  sourceRef,
  state,
  actions,
}: {
  entry: Entry | null
  onClose: () => void
  onNavigate: (delta: number) => void
  onOpen: (entry: Entry) => void
  sourceRef: RefObject<HTMLDivElement | null>
  state: PreviewDialogState
  actions: PreviewDialogActions
}) {
  const {
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
  } = state
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
    return fitPreviewMedia(media, window.innerWidth, window.innerHeight)
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
  const loadingPreview = Boolean(!error && declaresLoading && !previewReady)
  const waitingForLoadingDocument = Boolean(loadingPreview && !loadingPage)
  const pluginSheet = view?.kind === 'html' || loadingPreview
  const loadingPlaceholder = <div className="preview-plugin-state">{t('正在准备预览…')}</div>
  const selectProvider = (id: string) => {
    actions.setProviderId(id)
    actions.setProviderAttempt((value) => value + 1)
    actions.setMenuOpen(false)
  }

  return (
    <Modal
      open={!!entry && readyToShow}
      onClose={onClose}
      title={entry?.name || t('快速查看')}
      wide
      className={`preview-modal ${previewDialogClass(pluginSheet, view?.kind, Boolean(fit))}`}
      contentStyle={
        pluginSheet ? { width: 'min(92vw, 980px)' } : fit ? { width: fit.width } : undefined
      }
    >
      {entry && (
        <>
          <div className="preview-body" style={fit ? { height: fit.height } : undefined}>
            <PreviewContent
              entry={entry}
              view={view}
              fileSource={fileSource}
              highlighted={highlighted}
              error={error}
              loading={loading}
              resolved={resolved}
              loadingPage={loadingPage}
              waitingForLoadingDocument={waitingForLoadingDocument}
              loadingPlaceholder={loadingPlaceholder}
              providerId={providerId}
              previewAsset={previewAsset}
              previewReady={previewReady}
              onPreviewReady={() => actions.setPreviewReady(true)}
              onMediaChange={(value) => actions.setMedia(value)}
              onError={(message) => actions.setError(message)}
            />
          </div>
          <PreviewFooter
            entry={entry}
            candidates={candidates}
            providerId={providerId}
            providerLabel={providerLabel}
            fromPlugin={fromPlugin}
            menuOpen={menuOpen}
            sourceRef={sourceRef}
            onNavigate={onNavigate}
            onOpen={onOpen}
            onMenuOpenChange={actions.setMenuOpen}
            onProviderSelect={selectProvider}
          />
        </>
      )}
    </Modal>
  )
}

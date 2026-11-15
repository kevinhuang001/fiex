import type { Dispatch, RefObject, SetStateAction } from 'react'
import type { PreviewCandidate, ResolvedPreview } from '@/lib/preview'
import type { Entry } from '@/lib/types'

export interface PluginLoadingPage {
  pluginId: string
  /** Bridge-only session for the loading document; Core has no preview session yet. */
  sessionId: string
  src: string
}

export interface PreviewPreferences {
  order: string[]
  disabled: string[]
}

export type PreviewSessionState = {
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

export type PreviewSessionActions = {
  setPreviewReady: Dispatch<SetStateAction<boolean>>
  setMedia: Dispatch<SetStateAction<{ width: number; height: number } | null>>
  setError: Dispatch<SetStateAction<string>>
  setProviderId: Dispatch<SetStateAction<string | null>>
  setProviderAttempt: Dispatch<SetStateAction<number>>
  setMenuOpen: Dispatch<SetStateAction<boolean>>
}

export interface PreviewSessionServices {
  cachedCandidates(
    key: string,
    load: () => Promise<PreviewCandidate[]>,
  ): Promise<PreviewCandidate[]>
  findCandidates(path: string): Promise<PreviewCandidate[]>
  readPreferences(): PreviewPreferences
  loadingDocument(pluginId: string): Promise<{ src: string } | null>
  openPreview(path: string, providerId: string, providerOrder: string[]): Promise<ResolvedPreview>
  pluginViewAsset(pluginId: string, entry: string): Promise<string>
  toAssetUrl(path: string): string
  isNative: boolean
  noAvailablePreviewMessage(): string
  newSessionId(): string
  reportError(cause: unknown, source: string): void
  reportFilesystemPermissionError(cause: unknown, source: string): boolean
}

export interface PreviewSessionInput {
  entry: Entry | null
  onClose: () => void
  onNavigate: (delta: number) => void
}

export interface PreviewSessionResult {
  sourceRef: RefObject<HTMLDivElement | null>
  state: PreviewSessionState
  actions: PreviewSessionActions
}

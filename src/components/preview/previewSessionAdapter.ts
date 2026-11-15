import { t } from '@/i18n'
import { asset, call, native } from '@/lib/api'
import { MemoryCache } from '@/lib/cache'
import { reportError, reportFilesystemPermissionError } from '@/lib/errorCenter'
import { readPreviewPreferences } from '@/features/preview/services/preferences'
import type { PreviewSessionServices } from '@/features/preview/types'
import type { PreviewCandidate } from '@/lib/preview'

interface LoadingDocument {
  src: string
}

const candidateCache = new MemoryCache<PreviewCandidate[]>(64, 3_000)
const loadingDocumentCache = new MemoryCache<LoadingDocument | null>(16, 5 * 60_000)

function loadingDocument(pluginId: string): Promise<LoadingDocument | null> {
  return loadingDocumentCache.get(pluginId, async () => {
    const path = await call('preview_loading_asset', { pluginId })
    return path ? { src: asset(path) } : null
  })
}

/** Connect the preview feature to app-owned IPC, caches, and error reporting. */
export const previewSessionServices: PreviewSessionServices = {
  cachedCandidates: (key, load) => candidateCache.get(key, load),
  findCandidates: (path) => call('preview_candidates', { path }),
  readPreferences: readPreviewPreferences,
  loadingDocument,
  openPreview: (path, providerId, providerOrder) =>
    call('preview_open', { path, providerId, providerOrder }),
  pluginViewAsset: (pluginId, entry) => call('plugin_view_asset', { pluginId, entry }),
  toAssetUrl: asset,
  isNative: native,
  noAvailablePreviewMessage: () => t('没有可用的预览方式'),
  newSessionId: () => crypto.randomUUID(),
  reportError,
  reportFilesystemPermissionError,
}

import type { PluginLoadingPage, ResolvedPreview } from './types'
import { t } from '@/i18n'
import { lazy, Suspense } from 'react'
import type { ReactNode } from 'react'
import { AlertCircle } from 'lucide-react'
import { FileIcon } from '../FileIcon'
import { reportError } from '@/lib/errorCenter'
import { kind } from '@/lib/files'
import { PluginPreviewFrame } from '@/components/plugins/PluginPreviewFrame'
import type { Entry } from '@/lib/types'

const PdfPreview = lazy(() => import('../PdfPreview'))

type PreviewContentProps = {
  entry: Entry
  view: ResolvedPreview['view'] | undefined
  fileSource: string
  highlighted: string | null
  error: string
  loading: boolean
  resolved: ResolvedPreview | null
  loadingPage: PluginLoadingPage | null
  waitingForLoadingDocument: boolean
  loadingPlaceholder: ReactNode
  providerId: string | null
  previewAsset: string
  previewReady: boolean
  onPreviewReady: () => void
  onMediaChange: (media: { width: number; height: number }) => void
  onError: (message: string) => void
}

export function PreviewContent({
  entry,
  view,
  fileSource,
  highlighted,
  error,
  loading,
  resolved,
  loadingPage,
  waitingForLoadingDocument,
  loadingPlaceholder,
  providerId,
  previewAsset,
  previewReady,
  onPreviewReady,
  onMediaChange,
  onError,
}: PreviewContentProps) {
  const mediaError = (message: string) => {
    onError(message)
    reportError(message, 'preview:media')
  }

  if (error) {
    return (
      <div className="preview-fallback">
        <AlertCircle />
        <p>{t('无法预览')}</p>
        <FileIcon entry={entry} size={90} />
      </div>
    )
  }
  if (loadingPage && !resolved) {
    return (
      <PluginPreviewFrame
        key={`loading:${entry.path}:${providerId}:${loadingPage.sessionId}`}
        pluginId={loadingPage.pluginId}
        contributionId={loadingPage.pluginId}
        src={loadingPage.src}
        sessionId={loadingPage.sessionId}
        loading
      />
    )
  }
  if (view?.kind === 'html' && view.html) {
    if (waitingForLoadingDocument) return loadingPlaceholder
    return (
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
          onPreviewReady={onPreviewReady}
        />
        {loadingPage && !previewReady && (
          <div className="preview-plugin-loading">
            <PluginPreviewFrame
              key={`overlay:${entry.path}:${providerId}:${loadingPage.sessionId}`}
              pluginId={loadingPage.pluginId}
              contributionId={loadingPage.pluginId}
              src={loadingPage.src}
              sessionId={loadingPage.sessionId}
              loading
            />
          </div>
        )}
      </>
    )
  }
  if (waitingForLoadingDocument) return loadingPlaceholder
  if (loading && !resolved) return <p>{t('正在读取…')}</p>
  if (view?.kind === 'text') {
    return (
      <pre className={highlighted ? 'preview-code' : undefined}>
        {highlighted ? (
          <code className="hljs" dangerouslySetInnerHTML={{ __html: highlighted }} />
        ) : (
          view.text || t('（空文件）')
        )}
        {view.truncated && t('\n\n…仅预览前 256 KiB')}
      </pre>
    )
  }
  if (view?.kind === 'image') {
    return (
      <img
        src={fileSource}
        alt={entry.name}
        onLoad={(event) =>
          onMediaChange({
            width: event.currentTarget.naturalWidth,
            height: event.currentTarget.naturalHeight,
          })
        }
        onError={() => mediaError(t('此图片格式无法预览，可用默认应用打开。'))}
      />
    )
  }
  if (view?.kind === 'media' && view.media === 'video') {
    return (
      <video
        controls
        autoPlay
        src={fileSource}
        onLoadedMetadata={(event) =>
          onMediaChange({
            width: event.currentTarget.videoWidth,
            height: event.currentTarget.videoHeight,
          })
        }
        onError={() => mediaError(t('系统不支持此视频编码，可用默认应用打开。'))}
      />
    )
  }
  if (view?.kind === 'media') {
    return (
      <div className="preview-fallback">
        <FileIcon entry={entry} size={100} />
        <audio
          controls
          autoPlay
          src={fileSource}
          onError={() => mediaError(t('系统不支持此音频编码，可用默认应用打开。'))}
        />
      </div>
    )
  }
  if (view?.kind === 'pdf' && fileSource) {
    return (
      <Suspense fallback={<p>{t('正在加载 PDF…')}</p>}>
        <PdfPreview url={fileSource} />
      </Suspense>
    )
  }
  return (
    <div className="preview-fallback">
      <FileIcon entry={entry} size={120} />
      <h3>{view?.name || entry.name}</h3>
      <p>{view?.message || kind(entry)}</p>
    </div>
  )
}

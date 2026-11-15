import type { Entry } from '@/lib/types'
import { usePreviewSession } from '@/features/preview/hooks/usePreviewSession'
import { previewSessionServices } from './previewSessionAdapter'
import { PluginPreviewFrame } from '@/components/plugins/PluginPreviewFrame'
import { PreviewDialog } from '@/features/preview/components/PreviewDialog'

export function Preview({
  entry,
  onClose,
  onNavigate,
  onOpen,
}: {
  entry: Entry | null
  onClose: () => void
  onNavigate: (delta: number) => void
  onOpen: (entry: Entry) => void
}) {
  const { sourceRef, state, actions } = usePreviewSession(
    { entry, onClose, onNavigate },
    previewSessionServices,
  )
  const path = state.resolved?.view?.path
  const fileSource = path
    ? previewSessionServices.isNative
      ? previewSessionServices.toAssetUrl(path)
      : path
    : ''
  return (
    <PreviewDialog
      entry={entry}
      onClose={onClose}
      onNavigate={onNavigate}
      onOpen={onOpen}
      sourceRef={sourceRef}
      state={state}
      actions={actions}
      fileSource={fileSource}
      PluginFrame={PluginPreviewFrame}
      onReportError={previewSessionServices.reportError}
    />
  )
}

import type { Entry } from '@/lib/types'
import { usePreviewSession } from '@/features/preview/hooks/usePreviewSession'
import { previewSessionServices } from './preview/previewSessionAdapter'
import { PreviewDialog } from './preview/PreviewDialog'

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
  return (
    <PreviewDialog
      entry={entry}
      onClose={onClose}
      onNavigate={onNavigate}
      onOpen={onOpen}
      sourceRef={sourceRef}
      state={state}
      actions={actions}
    />
  )
}

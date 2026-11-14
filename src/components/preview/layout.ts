export type PreviewMediaSize = { width: number; height: number }
export type PreviewDialogSize = { width: number; height: number }
export type PreviewViewKind = 'html' | 'text' | 'image' | 'media' | 'pdf' | 'fallback'

export function fitPreviewMedia(
  media: PreviewMediaSize | null,
  viewportWidth: number,
  viewportHeight: number,
): PreviewDialogSize | null {
  if (!media || !media.width || !media.height) return null
  const maxWidth = Math.round(viewportWidth * 0.92)
  const maxHeight = Math.round(viewportHeight * 0.76)
  const scale = Math.min(1, maxWidth / media.width, maxHeight / media.height)
  return {
    width: Math.max(320, Math.round(media.width * scale)),
    height: Math.round(media.height * scale),
  }
}

export function previewDialogClass(
  pluginSheet: boolean,
  viewKind: PreviewViewKind | undefined,
  hasFit: boolean,
): string {
  if (pluginSheet) return 'preview-plugin'
  if (viewKind === 'text') return 'preview-text'
  if (viewKind === 'pdf') return 'preview-pdf'
  if (hasFit) return 'preview-media'
  return 'preview-compact'
}

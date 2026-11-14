import { describe, expect, it } from 'vitest'
import { fitPreviewMedia, previewDialogClass } from './layout'

describe('fitPreviewMedia', () => {
  it('does not size a preview before media dimensions are known', () => {
    expect(fitPreviewMedia(null, 1200, 900)).toBeNull()
    expect(fitPreviewMedia({ width: 0, height: 400 }, 1200, 900)).toBeNull()
  })

  it('keeps media at its natural size when it fits the viewport', () => {
    expect(fitPreviewMedia({ width: 640, height: 480 }, 1200, 900)).toEqual({
      width: 640,
      height: 480,
    })
  })

  it('scales large media and preserves the minimum preview width for small media', () => {
    expect(fitPreviewMedia({ width: 2400, height: 1600 }, 1200, 1200)).toEqual({
      width: 1104,
      height: 736,
    })
    expect(fitPreviewMedia({ width: 240, height: 120 }, 1200, 900)).toEqual({
      width: 320,
      height: 120,
    })
  })
})

describe('previewDialogClass', () => {
  it('prioritizes the plugin sheet over the resolved view kind', () => {
    expect(previewDialogClass(true, 'text', false)).toBe('preview-plugin')
  })

  it('selects content-specific and fallback classes', () => {
    expect(previewDialogClass(false, 'text', false)).toBe('preview-text')
    expect(previewDialogClass(false, 'pdf', false)).toBe('preview-pdf')
    expect(previewDialogClass(false, 'image', true)).toBe('preview-media')
    expect(previewDialogClass(false, 'fallback', false)).toBe('preview-compact')
    expect(previewDialogClass(false, undefined, false)).toBe('preview-compact')
  })
})

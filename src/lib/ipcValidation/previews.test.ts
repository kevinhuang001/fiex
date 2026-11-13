import { describe, expect, it } from 'vitest'
import { parsePreviewCandidates, parseResolvedPreview } from './previews'

describe('preview IPC validation', () => {
  it('constructs preview candidates and omits absent loading flags', () => {
    expect(
      parsePreviewCandidates([
        { id: 'builtin:text', provider: 'Text', priority: 10 },
        { id: 'plugin:org.example', provider: 'Example', priority: 50, loading: true },
      ]),
    ).toEqual([
      { id: 'builtin:text', provider: 'Text', priority: 10 },
      { id: 'plugin:org.example', provider: 'Example', priority: 50, loading: true },
    ])
  })

  it('decodes each supported resolved preview view shape', () => {
    const views = [
      {
        kind: 'html',
        html: '<main/>',
        entry: 'index.html',
        pluginId: 'org.example',
        sessionId: 's1',
        contributionId: 'org.example',
      },
      { kind: 'text', text: 'hello', truncated: false, size: 5 },
      { kind: 'image', path: '/image.png', name: 'image.png', mime: 'image/png' },
      {
        kind: 'media',
        media: 'video',
        path: '/clip.mp4',
        name: 'clip.mp4',
        mime: 'video/mp4',
      },
      { kind: 'pdf', path: '/document.pdf', name: 'document.pdf' },
      { kind: 'fallback', name: 'unknown', extension: 'bin' },
    ]
    for (const view of views) {
      expect(
        parseResolvedPreview({ providerId: 'builtin:test', provider: 'Test', priority: 1, view }),
      ).toEqual({ providerId: 'builtin:test', provider: 'Test', priority: 1, view })
    }
  })

  it.each([
    { kind: 'unknown' },
    { kind: 'image', path: '/image.png', name: 'image.png' },
    { kind: 'media', media: ['video'], path: '/clip.mp4', name: 'clip.mp4', mime: 'video/mp4' },
    { kind: 'media', media: 'animation', path: '/clip.mp4', name: 'clip.mp4', mime: 'video/mp4' },
    { kind: 'html', html: '<main/>', entry: 'index.html', pluginId: 'org.example' },
    { kind: 'text', text: 'hello', unexpected: true },
    { kind: 'text', text: 'hello', size: '5' },
  ])('rejects malformed resolved preview views', (view) => {
    expect(() =>
      parseResolvedPreview({ providerId: 'provider', provider: 'Provider', priority: 1, view }),
    ).toThrow('Invalid preview response')
  })

  it('rejects unknown preview candidate fields and malformed envelopes', () => {
    expect(() =>
      parsePreviewCandidates([{ id: 'test', provider: 'Test', priority: 1, extra: true }]),
    ).toThrow('Invalid preview candidate')
    expect(() =>
      parseResolvedPreview({
        providerId: 'test',
        provider: 'Test',
        priority: 1,
        view: { kind: 'fallback', name: 'file', extension: 'bin' },
        extra: true,
      }),
    ).toThrow('Invalid preview response')
  })
})

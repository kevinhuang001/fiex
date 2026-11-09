import type { PreviewCandidate } from '../preview'
import type { ResolvedPreview } from '@/components/preview/types'
import { objectResult, parseJsonValue } from './json'

export function parsePreviewCandidates(value: unknown): PreviewCandidate[] {
  const json = parseJsonValue(value)
  if (!Array.isArray(json)) throw new TypeError('Invalid preview candidates')
  for (const candidate of json) {
    if (
      candidate === null ||
      Array.isArray(candidate) ||
      typeof candidate !== 'object' ||
      typeof candidate.id !== 'string' ||
      typeof candidate.provider !== 'string' ||
      typeof candidate.priority !== 'number' ||
      (candidate.loading !== undefined && typeof candidate.loading !== 'boolean')
    )
      throw new TypeError('Invalid preview candidate')
  }
  return json as unknown as PreviewCandidate[]
}

export function parseResolvedPreview(value: unknown): ResolvedPreview {
  const result = objectResult(value)
  const view = result.view
  if (
    typeof result.providerId !== 'string' ||
    typeof result.provider !== 'string' ||
    typeof result.priority !== 'number' ||
    view === null ||
    Array.isArray(view) ||
    typeof view !== 'object' ||
    !['html', 'text', 'image', 'media', 'pdf', 'fallback'].includes(String(view.kind))
  )
    throw new TypeError('Invalid preview response')
  return result as unknown as ResolvedPreview
}

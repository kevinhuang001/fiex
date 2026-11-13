import type { PreviewCandidate } from '../preview'
import type { ResolvedPreview } from '@/components/preview/types'
import { isJsonObject, objectResult, parseJsonValue } from './json'

const PREVIEW_KINDS = ['html', 'text', 'image', 'media', 'pdf', 'fallback'] as const

function onlyKeys(value: Record<string, unknown>, allowed: string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key))
}

function stringField(value: Record<string, unknown>, key: string): string | undefined {
  return typeof value[key] === 'string' ? value[key] : undefined
}

function numberField(value: Record<string, unknown>, key: string): number | undefined {
  return typeof value[key] === 'number' ? value[key] : undefined
}

function hasInvalidOptionalFields(
  value: Record<string, unknown>,
  strings: string[],
  booleans: string[],
  numbers: string[],
): boolean {
  return (
    strings.some((key) => value[key] !== undefined && typeof value[key] !== 'string') ||
    booleans.some((key) => value[key] !== undefined && typeof value[key] !== 'boolean') ||
    numbers.some((key) => {
      const number = value[key]
      return (
        number !== undefined &&
        (typeof number !== 'number' || !Number.isSafeInteger(number) || number < 0)
      )
    })
  )
}

function isPreviewKind(value: unknown): value is ResolvedPreview['view']['kind'] {
  return PREVIEW_KINDS.some((kind) => kind === value)
}

function parsePreviewView(value: unknown): ResolvedPreview['view'] {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, [
      'kind',
      'html',
      'entry',
      'text',
      'truncated',
      'path',
      'name',
      'mime',
      'media',
      'message',
      'sessionId',
      'pluginId',
      'contributionId',
      'timeoutMs',
      'context',
      'state',
      'size',
      'extension',
    ]) ||
    !isPreviewKind(value.kind) ||
    hasInvalidOptionalFields(
      value,
      [
        'html',
        'entry',
        'text',
        'path',
        'name',
        'mime',
        'media',
        'message',
        'sessionId',
        'pluginId',
        'contributionId',
        'extension',
      ],
      ['truncated'],
      ['timeoutMs', 'size'],
    )
  )
    throw new TypeError('Invalid preview response')

  const kind = value.kind
  const html = stringField(value, 'html')
  const entry = stringField(value, 'entry')
  const text = stringField(value, 'text')
  const path = stringField(value, 'path')
  const name = stringField(value, 'name')
  const mime = stringField(value, 'mime')
  const media = stringField(value, 'media')
  const message = stringField(value, 'message')
  const sessionId = stringField(value, 'sessionId')
  const pluginId = stringField(value, 'pluginId')
  const contributionId = stringField(value, 'contributionId')
  const extension = stringField(value, 'extension')
  const truncated = typeof value.truncated === 'boolean' ? value.truncated : undefined
  const timeoutMs = numberField(value, 'timeoutMs')
  const size = numberField(value, 'size')
  if (media !== undefined && media !== 'video' && media !== 'audio')
    throw new TypeError('Invalid preview response')

  if (
    (kind === 'html' &&
      (html === undefined || !entry || !pluginId || !sessionId || !contributionId)) ||
    (kind === 'text' && text === undefined) ||
    (kind === 'image' && (!path || name === undefined || mime === undefined)) ||
    (kind === 'media' &&
      (!path ||
        name === undefined ||
        mime === undefined ||
        !['video', 'audio'].includes(media ?? ''))) ||
    (kind === 'pdf' && (!path || name === undefined)) ||
    (kind === 'fallback' && (name === undefined || extension === undefined))
  )
    throw new TypeError('Invalid preview response')

  return {
    kind,
    ...(html !== undefined ? { html } : {}),
    ...(entry !== undefined ? { entry } : {}),
    ...(text !== undefined ? { text } : {}),
    ...(truncated !== undefined ? { truncated } : {}),
    ...(path !== undefined ? { path } : {}),
    ...(name !== undefined ? { name } : {}),
    ...(mime !== undefined ? { mime } : {}),
    ...(media === 'video' || media === 'audio' ? { media } : {}),
    ...(message !== undefined ? { message } : {}),
    ...(sessionId !== undefined ? { sessionId } : {}),
    ...(pluginId !== undefined ? { pluginId } : {}),
    ...(contributionId !== undefined ? { contributionId } : {}),
    ...(timeoutMs !== undefined ? { timeoutMs } : {}),
    ...(value.context !== undefined ? { context: value.context } : {}),
    ...(value.state !== undefined ? { state: value.state } : {}),
    ...(size !== undefined ? { size } : {}),
    ...(extension !== undefined ? { extension } : {}),
  }
}

export function parsePreviewCandidates(value: unknown): PreviewCandidate[] {
  const json = parseJsonValue(value)
  if (!Array.isArray(json)) throw new TypeError('Invalid preview candidates')
  const candidates: PreviewCandidate[] = []
  for (const candidate of json) {
    if (
      !isJsonObject(candidate) ||
      !onlyKeys(candidate, ['id', 'provider', 'priority', 'loading']) ||
      typeof candidate.id !== 'string' ||
      typeof candidate.provider !== 'string' ||
      typeof candidate.priority !== 'number' ||
      !Number.isFinite(candidate.priority) ||
      (candidate.loading !== undefined && typeof candidate.loading !== 'boolean')
    )
      throw new TypeError('Invalid preview candidate')
    candidates.push({
      id: candidate.id,
      provider: candidate.provider,
      priority: candidate.priority,
      ...(typeof candidate.loading === 'boolean' ? { loading: candidate.loading } : {}),
    })
  }
  return candidates
}

export function parseResolvedPreview(value: unknown): ResolvedPreview {
  const result = objectResult(value)
  if (
    !onlyKeys(result, ['providerId', 'provider', 'priority', 'view']) ||
    typeof result.providerId !== 'string' ||
    typeof result.provider !== 'string' ||
    typeof result.priority !== 'number' ||
    !Number.isFinite(result.priority)
  )
    throw new TypeError('Invalid preview response')
  return {
    providerId: result.providerId,
    provider: result.provider,
    priority: result.priority,
    view: parsePreviewView(result.view),
  }
}

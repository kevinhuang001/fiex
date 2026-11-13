import type { JsonValue } from '../../../sdk/ui/src/index'
import type {
  InfoField,
  InfoFieldDelta,
  InfoFieldUpdate,
  InfoOpenResult,
  InfoSection,
} from './infoTypes'
import { objectResult } from './json'

const FIELD_STATUSES = [
  'idle',
  'pending',
  'ready',
  'unavailable',
  'error',
] satisfies InfoField['status'][]
const COMPUTE_MODES = ['onOpen', 'onDemand'] satisfies InfoField['compute'][]
const SECTION_SOURCES = ['core', 'plugin'] satisfies InfoSection['source'][]

export function shouldApplyInfoDelta(
  delta: { sessionId: string; revision: number },
  activeSession: string,
  currentRevision: number,
): boolean {
  return delta.sessionId === activeSession && delta.revision > currentRevision
}

function record(value: JsonValue): value is { [key: string]: JsonValue } {
  return value !== null && !Array.isArray(value) && typeof value === 'object'
}

function onlyKeys(value: { [key: string]: JsonValue }, allowed: string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key))
}

function parseInfoField(value: JsonValue): InfoField | null {
  if (
    !record(value) ||
    !onlyKeys(value, ['id', 'subsectionId', 'label', 'compute', 'status', 'value', 'error'])
  )
    return null
  if (
    typeof value.id === 'string' &&
    value.id.length > 0 &&
    typeof value.subsectionId === 'string' &&
    value.subsectionId.length > 0 &&
    typeof value.label === 'string' &&
    value.label.length > 0 &&
    COMPUTE_MODES.some((mode) => mode === value.compute) &&
    FIELD_STATUSES.some((status) => status === value.status) &&
    (value.value === undefined || typeof value.value === 'string') &&
    (value.error === undefined || typeof value.error === 'string')
  ) {
    return {
      id: value.id,
      subsectionId: value.subsectionId,
      label: value.label,
      compute: value.compute as InfoField['compute'],
      status: value.status as InfoField['status'],
      ...(typeof value.value === 'string' ? { value: value.value } : {}),
      ...(typeof value.error === 'string' ? { error: value.error } : {}),
    }
  }
  return null
}

function parseInfoSection(value: JsonValue): InfoSection | null {
  if (
    !record(value) ||
    !onlyKeys(value, [
      'id',
      'title',
      'providerId',
      'provider',
      'source',
      'pluginId',
      'fields',
      'error',
    ]) ||
    typeof value.id !== 'string' ||
    typeof value.title !== 'string' ||
    !value.title ||
    typeof value.providerId !== 'string' ||
    !value.providerId ||
    value.id !== value.providerId ||
    typeof value.provider !== 'string' ||
    !SECTION_SOURCES.some((source) => source === value.source) ||
    (value.pluginId !== undefined && typeof value.pluginId !== 'string') ||
    (value.error !== undefined && typeof value.error !== 'string') ||
    !Array.isArray(value.fields)
  )
    return null

  const fields: InfoField[] = []
  const fieldIds = new Set<string>()
  for (const rawField of value.fields) {
    const field = parseInfoField(rawField)
    if (!field) return null
    const key = `${field.subsectionId}\u0000${field.id}`
    if (fieldIds.has(key)) return null
    fieldIds.add(key)
    fields.push(field)
  }

  return {
    id: value.id,
    title: value.title,
    providerId: value.providerId,
    provider: value.provider,
    source: value.source as InfoSection['source'],
    ...(typeof value.pluginId === 'string' ? { pluginId: value.pluginId } : {}),
    fields,
    ...(typeof value.error === 'string' ? { error: value.error } : {}),
  }
}

function parseInfoFieldUpdate(value: JsonValue): InfoFieldUpdate | null {
  if (
    !record(value) ||
    !onlyKeys(value, ['providerId', 'subsectionId', 'fieldId', 'status', 'value', 'error']) ||
    typeof value.providerId !== 'string' ||
    !value.providerId ||
    typeof value.subsectionId !== 'string' ||
    !value.subsectionId ||
    typeof value.fieldId !== 'string' ||
    !value.fieldId ||
    !FIELD_STATUSES.some((status) => status === value.status) ||
    (value.value !== undefined && typeof value.value !== 'string') ||
    (value.error !== undefined && typeof value.error !== 'string')
  )
    return null

  return {
    providerId: value.providerId,
    subsectionId: value.subsectionId,
    fieldId: value.fieldId,
    status: value.status as InfoField['status'],
    ...(typeof value.value === 'string' ? { value: value.value } : {}),
    ...(typeof value.error === 'string' ? { error: value.error } : {}),
  }
}

export function parseInfoOpenResult(value: unknown): InfoOpenResult {
  const result = objectResult(value)
  const providerIds = new Set<string>()
  if (
    !onlyKeys(result, ['sessionId', 'sections']) ||
    typeof result.sessionId !== 'string' ||
    !result.sessionId ||
    !Array.isArray(result.sections)
  )
    throw new TypeError('Invalid file info response')

  const sections: InfoSection[] = []
  for (const rawSection of result.sections) {
    const section = parseInfoSection(rawSection)
    if (!section || providerIds.has(section.providerId))
      throw new TypeError('Invalid file info response')
    providerIds.add(section.providerId)
    sections.push(section)
  }
  return { sessionId: result.sessionId, sections }
}

export function parseInfoFieldDelta(value: unknown): InfoFieldDelta {
  const delta = objectResult(value)
  const keys = new Set<string>()
  if (
    !onlyKeys(delta, ['sessionId', 'revision', 'updates']) ||
    typeof delta.sessionId !== 'string' ||
    !delta.sessionId ||
    typeof delta.revision !== 'number' ||
    !Number.isSafeInteger(delta.revision) ||
    delta.revision < 0 ||
    !Array.isArray(delta.updates)
  )
    throw new TypeError('Invalid file info update')

  const updates: InfoFieldUpdate[] = []
  for (const rawUpdate of delta.updates) {
    const update = parseInfoFieldUpdate(rawUpdate)
    if (!update) throw new TypeError('Invalid file info update')
    const key = `${update.providerId}\u0000${update.subsectionId}\u0000${update.fieldId}`
    if (keys.has(key)) throw new TypeError('Invalid file info update')
    keys.add(key)
    updates.push(update)
  }
  return { sessionId: delta.sessionId, revision: delta.revision, updates }
}

import type { JsonValue } from '../../../sdk/ui/src/index'
import type { InfoFieldDelta, InfoOpenResult } from '@/components/FileInfoDialog'
import { objectResult } from './json'

const FIELD_STATUSES = ['idle', 'pending', 'ready', 'unavailable', 'error']
const COMPUTE_MODES = ['onOpen', 'onDemand']

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

function infoField(value: JsonValue): boolean {
  if (
    !record(value) ||
    !onlyKeys(value, ['id', 'subsectionId', 'label', 'compute', 'status', 'value', 'error'])
  )
    return false
  return (
    typeof value.id === 'string' &&
    value.id.length > 0 &&
    typeof value.subsectionId === 'string' &&
    value.subsectionId.length > 0 &&
    typeof value.label === 'string' &&
    value.label.length > 0 &&
    COMPUTE_MODES.includes(String(value.compute)) &&
    FIELD_STATUSES.includes(String(value.status)) &&
    (value.value === undefined || typeof value.value === 'string') &&
    (value.error === undefined || typeof value.error === 'string')
  )
}

export function parseInfoOpenResult(value: unknown): InfoOpenResult {
  const result = objectResult(value)
  const providerIds = new Set<string>()
  if (
    typeof result.sessionId !== 'string' ||
    !result.sessionId ||
    !Array.isArray(result.sections) ||
    !result.sections.every((rawSection) => {
      if (
        !record(rawSection) ||
        !onlyKeys(rawSection, [
          'id',
          'title',
          'providerId',
          'provider',
          'source',
          'pluginId',
          'fields',
          'error',
        ]) ||
        typeof rawSection.id !== 'string' ||
        typeof rawSection.title !== 'string' ||
        !rawSection.title ||
        typeof rawSection.providerId !== 'string' ||
        !rawSection.providerId ||
        rawSection.id !== rawSection.providerId ||
        providerIds.has(rawSection.providerId) ||
        typeof rawSection.provider !== 'string' ||
        !['core', 'plugin'].includes(String(rawSection.source)) ||
        (rawSection.pluginId !== undefined && typeof rawSection.pluginId !== 'string') ||
        (rawSection.error !== undefined && typeof rawSection.error !== 'string') ||
        !Array.isArray(rawSection.fields) ||
        !rawSection.fields.every(infoField)
      )
        return false
      const fieldIds = new Set<string>()
      for (const field of rawSection.fields) {
        if (!record(field)) return false
        const key = `${field.subsectionId}\u0000${field.id}`
        if (fieldIds.has(key)) return false
        fieldIds.add(key)
      }
      providerIds.add(rawSection.providerId)
      return true
    })
  )
    throw new TypeError('Invalid file info response')
  return result as unknown as InfoOpenResult
}

export function parseInfoFieldDelta(value: unknown): InfoFieldDelta {
  const delta = objectResult(value)
  const keys = new Set<string>()
  if (
    typeof delta.sessionId !== 'string' ||
    !delta.sessionId ||
    typeof delta.revision !== 'number' ||
    !Number.isSafeInteger(delta.revision) ||
    delta.revision < 0 ||
    !Array.isArray(delta.updates) ||
    !delta.updates.every((rawUpdate) => {
      if (
        !record(rawUpdate) ||
        !onlyKeys(rawUpdate, [
          'providerId',
          'subsectionId',
          'fieldId',
          'status',
          'value',
          'error',
        ]) ||
        typeof rawUpdate.providerId !== 'string' ||
        !rawUpdate.providerId ||
        typeof rawUpdate.subsectionId !== 'string' ||
        !rawUpdate.subsectionId ||
        typeof rawUpdate.fieldId !== 'string' ||
        !rawUpdate.fieldId ||
        !FIELD_STATUSES.includes(String(rawUpdate.status)) ||
        (rawUpdate.value !== undefined && typeof rawUpdate.value !== 'string') ||
        (rawUpdate.error !== undefined && typeof rawUpdate.error !== 'string')
      )
        return false
      const key = `${rawUpdate.providerId}\u0000${rawUpdate.subsectionId}\u0000${rawUpdate.fieldId}`
      if (keys.has(key)) return false
      keys.add(key)
      return true
    })
  )
    throw new TypeError('Invalid file info update')
  return delta as unknown as InfoFieldDelta
}

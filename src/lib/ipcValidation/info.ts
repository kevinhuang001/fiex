import type { JsonValue } from '../../../sdk/ui/src/index'
import type { InfoFieldDelta, InfoOpenResult } from '@/components/FileInfoDialog'
import { objectResult } from './json'

function infoField(value: JsonValue): boolean {
  return (
    value !== null &&
    !Array.isArray(value) &&
    typeof value === 'object' &&
    typeof value.id === 'string' &&
    typeof value.sectionId === 'string' &&
    typeof value.label === 'string' &&
    ['onOpen', 'onDemand'].includes(String(value.compute)) &&
    ['idle', 'pending', 'ready', 'unavailable', 'error'].includes(String(value.status)) &&
    (value.value === undefined || typeof value.value === 'string') &&
    (value.error === undefined || typeof value.error === 'string')
  )
}

export function parseInfoOpenResult(value: unknown): InfoOpenResult {
  const result = objectResult(value)
  if (
    typeof result.sessionId !== 'string' ||
    !Array.isArray(result.sections) ||
    !result.sections.every((section) => {
      if (
        section === null ||
        Array.isArray(section) ||
        typeof section !== 'object' ||
        typeof section.id !== 'string' ||
        typeof section.title !== 'string' ||
        typeof section.providerId !== 'string' ||
        typeof section.provider !== 'string' ||
        !['core', 'plugin'].includes(String(section.source)) ||
        !Array.isArray(section.fields) ||
        !section.fields.every(infoField)
      )
        return false
      return section.fields.every(
        (field) =>
          field === null ||
          Array.isArray(field) ||
          typeof field !== 'object' ||
          (field.sectionId === section.id &&
            (section.pluginId === undefined || field.providerId === section.providerId)),
      )
    })
  )
    throw new TypeError('Invalid file info response')
  return result as unknown as InfoOpenResult
}

export function parseInfoFieldDelta(value: unknown): InfoFieldDelta {
  const delta = objectResult(value)
  if (
    typeof delta.sessionId !== 'string' ||
    typeof delta.revision !== 'number' ||
    !Array.isArray(delta.updates) ||
    !delta.updates.every(
      (update) =>
        update !== null &&
        !Array.isArray(update) &&
        typeof update === 'object' &&
        typeof update.providerId === 'string' &&
        typeof update.sectionId === 'string' &&
        typeof update.fieldId === 'string' &&
        ['idle', 'pending', 'ready', 'unavailable', 'error'].includes(String(update.status)) &&
        (update.value === undefined || typeof update.value === 'string') &&
        (update.error === undefined || typeof update.error === 'string'),
    )
  )
    throw new TypeError('Invalid file info update')
  return delta as unknown as InfoFieldDelta
}

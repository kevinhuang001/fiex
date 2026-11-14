import type { OpenWithPickerResponse } from '../openWithPicker'
import { isJsonObject, onlyKeys } from './json'

export function parseOpenWithPickerResponse(value: unknown): OpenWithPickerResponse {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['sessionId', 'items']) ||
    typeof value.sessionId !== 'string' ||
    !value.sessionId ||
    !Array.isArray(value.items)
  )
    throw new TypeError('Invalid open_with_picker_candidates IPC result')

  const items = value.items.map((item) => {
    if (
      !isJsonObject(item) ||
      !onlyKeys(item, ['id', 'name', 'iconSource']) ||
      typeof item.id !== 'string' ||
      !item.id ||
      typeof item.name !== 'string' ||
      !item.name ||
      (item.iconSource !== null && typeof item.iconSource !== 'string')
    )
      throw new TypeError('Invalid open_with_picker_candidates IPC result')
    return { id: item.id, name: item.name, iconSource: item.iconSource }
  })

  return { sessionId: value.sessionId, items }
}

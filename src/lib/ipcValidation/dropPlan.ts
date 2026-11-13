import { parseResourceRef } from '../../../sdk/ui/src/resources.js'
import type { DropPlan } from '../types'
import { isJsonObject, onlyKeys } from './json'

function parseReferences(value: unknown): DropPlan['references'] {
  if (!Array.isArray(value)) throw new TypeError('Invalid internal_drop IPC result')
  return value.map((reference) => {
    if (
      !isJsonObject(reference) ||
      !onlyKeys(reference, ['mountId', 'nodeId']) ||
      typeof reference.mountId !== 'string' ||
      !reference.mountId ||
      typeof reference.nodeId !== 'string' ||
      !reference.nodeId
    )
      throw new TypeError('Invalid internal_drop IPC result')
    return parseResourceRef(reference)
  })
}

export function parseDropPlan(value: unknown): DropPlan | null {
  if (value === null) return null
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['paths', 'moving', 'references']) ||
    typeof value.moving !== 'boolean' ||
    !Array.isArray(value.paths) ||
    !value.paths.length ||
    !value.paths.every((path): path is string => typeof path === 'string' && path.length > 0)
  )
    throw new TypeError('Invalid internal_drop IPC result')

  const references = parseReferences(value.references)
  if (references.length !== value.paths.length)
    throw new TypeError('Invalid internal_drop IPC result')
  return { paths: value.paths, moving: value.moving, references }
}

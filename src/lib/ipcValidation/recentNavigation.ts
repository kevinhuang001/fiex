import type { RecentNavigationTarget } from '../types'
import { isJsonObject, onlyKeys } from './json'

export function parseRecentNavigationTarget(value: unknown): RecentNavigationTarget {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['parent', 'target']) ||
    typeof value.parent !== 'string' ||
    !value.parent ||
    typeof value.target !== 'string' ||
    !value.target
  )
    throw new TypeError('Invalid recent_navigation_target IPC result')

  return { parent: value.parent, target: value.target }
}

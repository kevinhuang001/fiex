import type { AppErrorEvent } from '../types'
import { isJsonObject, isUnsignedInteger, onlyKeys } from './json'

const APP_ERROR_KEYS = [
  'id',
  'at',
  'source',
  'code',
  'message',
  'operation',
  'retryable',
  'occurrences',
  'lastAt',
]

function isSafeUnsignedInteger(value: unknown): value is number {
  return isUnsignedInteger(value) && Number.isSafeInteger(value)
}

function parseAppError(value: unknown): AppErrorEvent {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, APP_ERROR_KEYS) ||
    !isSafeUnsignedInteger(value.id) ||
    value.id === 0 ||
    !isSafeUnsignedInteger(value.at) ||
    typeof value.source !== 'string' ||
    !value.source ||
    typeof value.code !== 'string' ||
    !value.code ||
    typeof value.message !== 'string' ||
    !value.message ||
    !(value.operation === null || typeof value.operation === 'string') ||
    typeof value.retryable !== 'boolean' ||
    !isSafeUnsignedInteger(value.occurrences) ||
    value.occurrences === 0 ||
    value.occurrences > 0xffff_ffff ||
    !isSafeUnsignedInteger(value.lastAt)
  )
    throw new TypeError('Invalid errors_pending IPC result')

  return {
    id: value.id,
    at: value.at,
    source: value.source,
    code: value.code,
    message: value.message,
    operation: value.operation,
    retryable: value.retryable,
    occurrences: value.occurrences,
    lastAt: value.lastAt,
  }
}

export function parsePendingErrors(value: unknown): AppErrorEvent[] {
  if (!Array.isArray(value)) throw new TypeError('Invalid errors_pending IPC result')
  return value.map(parseAppError)
}

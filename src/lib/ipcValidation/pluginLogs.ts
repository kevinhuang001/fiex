import type { LogPage, LogRecord } from '../types'
import { isJsonObject, isUnsignedInteger, onlyKeys } from './json'

const LOG_RECORD_KEYS = [
  'id',
  'at',
  'source',
  'level',
  'operationId',
  'durationMs',
  'message',
  'detail',
]

function isSafeUnsignedInteger(value: unknown): value is number {
  return isUnsignedInteger(value) && Number.isSafeInteger(value)
}

function isLogLevel(value: unknown): value is LogRecord['level'] {
  return (
    value === 'trace' ||
    value === 'debug' ||
    value === 'info' ||
    value === 'warn' ||
    value === 'error'
  )
}

function parseLogRecord(value: unknown): LogRecord {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, LOG_RECORD_KEYS) ||
    !isSafeUnsignedInteger(value.id) ||
    value.id === 0 ||
    !isSafeUnsignedInteger(value.at) ||
    typeof value.source !== 'string' ||
    !value.source ||
    !isLogLevel(value.level) ||
    !(value.operationId === null || typeof value.operationId === 'string') ||
    !(value.durationMs === null || isSafeUnsignedInteger(value.durationMs)) ||
    typeof value.message !== 'string' ||
    !value.message ||
    (value.detail !== undefined && typeof value.detail !== 'string')
  )
    throw new TypeError('Invalid plugin_logs IPC result')

  return {
    id: value.id,
    at: value.at,
    source: value.source,
    level: value.level,
    operationId: value.operationId,
    durationMs: value.durationMs,
    message: value.message,
    ...(typeof value.detail === 'string' ? { detail: value.detail } : {}),
  }
}

export function parsePluginLogPage(value: unknown): LogPage {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['entries', 'cursor', 'hasMore']) ||
    !Array.isArray(value.entries) ||
    !isSafeUnsignedInteger(value.cursor) ||
    typeof value.hasMore !== 'boolean'
  )
    throw new TypeError('Invalid plugin_logs IPC result')

  return {
    entries: value.entries.map(parseLogRecord),
    cursor: value.cursor,
    hasMore: value.hasMore,
  }
}

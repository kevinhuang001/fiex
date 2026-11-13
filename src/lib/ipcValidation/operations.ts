import type { FileVersion, OperationResult, TransferConflict } from '../types'
import { isJsonObject, isUnsignedInteger, objectResult, onlyKeys } from './json'

function parseFileVersion(value: unknown): FileVersion {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['path', 'size', 'modified', 'isDir']) ||
    typeof value.path !== 'string' ||
    !isUnsignedInteger(value.size) ||
    !isUnsignedInteger(value.modified) ||
    typeof value.isDir !== 'boolean'
  )
    throw new TypeError('Invalid file version from IPC')
  return {
    path: value.path,
    size: value.size,
    modified: value.modified,
    isDir: value.isDir,
  }
}

function parseTransferConflict(value: unknown): TransferConflict {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['key', 'source', 'destination']) ||
    typeof value.key !== 'string'
  )
    throw new TypeError('Invalid file transfer conflict from IPC')
  return {
    key: value.key,
    source: parseFileVersion(value.source),
    destination: parseFileVersion(value.destination),
  }
}

export function parseOperationResult(value: unknown): OperationResult {
  const result = objectResult(value)
  if (
    !onlyKeys(result, ['paths', 'errors', 'skipped', 'conflicts']) ||
    !Array.isArray(result.paths) ||
    !result.paths.every((path) => typeof path === 'string') ||
    !Array.isArray(result.errors) ||
    !result.errors.every((error) => typeof error === 'string') ||
    !isUnsignedInteger(result.skipped) ||
    !Array.isArray(result.conflicts)
  )
    throw new TypeError('Invalid file operation result from IPC')
  return {
    paths: result.paths,
    errors: result.errors,
    skipped: result.skipped,
    conflicts: result.conflicts.map(parseTransferConflict),
  }
}

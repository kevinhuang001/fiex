import type { Ide, Terminal } from '../types'
import type { ShellChoice } from '@/components/terminal/TerminalPanel'
import { isJsonObject, onlyKeys } from './json'

function parseNamedItem(value: unknown, command: string): { id: string; name: string } {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['id', 'name']) ||
    typeof value.id !== 'string' ||
    !value.id ||
    typeof value.name !== 'string' ||
    !value.name
  )
    throw new TypeError(`Invalid ${command} IPC result`)
  return { id: value.id, name: value.name }
}

function parseShellItem(value: unknown): ShellChoice {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['id', 'name', 'path']) ||
    typeof value.id !== 'string' ||
    !value.id ||
    typeof value.name !== 'string' ||
    !value.name ||
    typeof value.path !== 'string' ||
    !value.path
  )
    throw new TypeError('Invalid detect_shells IPC result')
  return { id: value.id, name: value.name, path: value.path }
}

function parseNamedList<T extends { id: string; name: string }>(
  value: unknown,
  command: string,
  parse: (item: unknown) => T,
): T[] {
  if (!Array.isArray(value)) throw new TypeError(`Invalid ${command} IPC result`)
  return value.map(parse)
}

export function decodeRuntimeCatalogIpcResult(
  command: string,
  value: unknown,
): { value: unknown } | null {
  if (command === 'detect_ides')
    return { value: parseNamedList<Ide>(value, command, (item) => parseNamedItem(item, command)) }
  if (command === 'detect_terminals')
    return {
      value: parseNamedList<Terminal>(value, command, (item) => parseNamedItem(item, command)),
    }
  if (command === 'detect_shells')
    return { value: parseNamedList<ShellChoice>(value, command, parseShellItem) }
  return null
}

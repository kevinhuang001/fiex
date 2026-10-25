import { basename, parent } from './files'
import type { Entry } from './types'

export type RenameCase = 'keep' | 'lower' | 'upper' | 'title'
export type RenameExtension = 'keep' | 'lower' | 'upper' | 'remove'

export interface RenameOptions {
  template: string
  find: string
  replace: string
  regex: boolean
  applyToExtension: boolean
  caseMode: RenameCase
  extensionMode: RenameExtension
  start: number
  step: number
  padding: number
}

export const DEFAULT_RENAME_OPTIONS: RenameOptions = {
  template: '{original}',
  find: '',
  replace: '',
  regex: false,
  applyToExtension: false,
  caseMode: 'keep',
  extensionMode: 'keep',
  start: 1,
  step: 1,
  padding: 1,
}

export const RENAME_PRESETS = [
  { label: '原文件名', template: '{original}' },
  { label: '序号 - 名称', template: '{n} - {name}' },
  { label: '名称_序号', template: '{name}_{n}' },
  { label: '日期_名称', template: '{date}_{name}' },
]

export const RENAME_TOKENS = [
  { token: '{original}', label: '原名称（不含扩展名）' },
  { token: '{ext}', label: '扩展名（含点）' },
  { token: '{n}', label: '序号，按位数补零' },
  { token: '{n2}', label: '两位序号' },
  { token: '{n3}', label: '三位序号' },
  { token: '{date}', label: '修改日期 YYYY-MM-DD' },
  { token: '{year}', label: '年' },
  { token: '{month}', label: '月' },
  { token: '{day}', label: '日' },
  { token: '{folder}', label: '所在文件夹名' },
]

export function splitExtension(name: string, isDir: boolean) {
  const dot = isDir ? -1 : name.lastIndexOf('.')
  return {
    stem: dot > 0 ? name.slice(0, dot) : name,
    ext: dot > 0 ? name.slice(dot) : '',
  }
}

function titleCase(value: string) {
  return value.replace(/(^|[\s._-])(\p{L})/gu, (_, prefix: string, letter: string) => {
    return prefix + letter.toLocaleUpperCase()
  })
}

function applyCase(value: string, mode: RenameCase) {
  if (mode === 'lower') return value.toLocaleLowerCase()
  if (mode === 'upper') return value.toLocaleUpperCase()
  if (mode === 'title') return titleCase(value)
  return value
}

function replaceText(value: string, find: string, replace: string, regex: boolean) {
  if (!find) return value
  if (regex) return value.replace(new RegExp(find, 'gu'), replace)
  return value.split(find).join(replace)
}

function sequence(index: number, options: RenameOptions, digits?: number) {
  const value = options.start + index * options.step
  const width = digits ?? Math.max(1, Math.min(12, options.padding))
  return String(value).padStart(width, '0')
}

function dateParts(modified: number) {
  const date = new Date((modified || 0) * 1000)
  const year = String(date.getFullYear())
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return { date: `${year}-${month}-${day}`, year, month, day }
}

/** Build the target names in selection order. Throws on invalid regex. */
export function buildRenames(entries: Entry[], options: RenameOptions): string[] {
  return entries.map((entry, index) => {
    const original = entry.name
    let { stem, ext } = splitExtension(original, entry.isDir)
    let source = options.applyToExtension ? original : stem
    source = replaceText(source, options.find, options.replace, options.regex)
    if (options.applyToExtension) {
      const split = splitExtension(source, entry.isDir)
      stem = split.stem
      ext = split.ext
    } else {
      stem = source
    }
    const date = dateParts(entry.modified)
    const folder = basename(parent(entry.path))
    const values: Record<string, string> = {
      original: splitExtension(original, entry.isDir).stem,
      name: stem,
      stem,
      ext,
      full: original,
      folder,
      ...date,
    }
    let result = options.template.replace(/\{([a-zA-Z0-9]+)\}/g, (match, token: string) => {
      if (token === 'n') return sequence(index, options)
      if (/^n\d+$/.test(token)) {
        return sequence(index, options, Math.max(1, Math.min(12, Number(token.slice(1)))))
      }
      return values[token] ?? match
    })
    result = applyCase(result, options.caseMode)
    const templateHasExtension = /\{(?:ext|full)\}/.test(options.template)
    if (!entry.isDir && !templateHasExtension && options.extensionMode !== 'remove') {
      const suffix =
        options.extensionMode === 'lower'
          ? ext.toLocaleLowerCase()
          : options.extensionMode === 'upper'
            ? ext.toLocaleUpperCase()
            : ext
      result += suffix
    }
    return result.trim()
  })
}

const RESERVED = new Set([
  'con',
  'prn',
  'aux',
  'nul',
  ...Array.from({ length: 9 }, (_, index) => `com${index + 1}`),
  ...Array.from({ length: 9 }, (_, index) => `lpt${index + 1}`),
])

/** Per-path problems; no entry means the target is valid. */
export function validateRenames(entries: Entry[], names: string[]): Map<string, string> {
  const problems = new Map<string, string>()
  const seen = new Map<string, string>()
  for (const [index, entry] of entries.entries()) {
    const name = names[index] ?? ''
    if (!name) {
      problems.set(entry.path, '名称不能为空')
      continue
    }
    if (name === '.' || name === '..' || /[\\/\0]/.test(name)) {
      problems.set(entry.path, '名称包含无效字符')
      continue
    }
    if (/[<>:"|?*]/.test(name) || /[. ]$/.test(name)) {
      problems.set(entry.path, '名称包含 Windows 不允许的字符')
      continue
    }
    const stem = entry.isDir ? name : splitExtension(name, false).stem
    if (RESERVED.has(stem.toLocaleLowerCase())) {
      problems.set(entry.path, '这是 Windows 保留名称')
      continue
    }
    const key = name.toLocaleLowerCase()
    const previous = seen.get(key)
    if (previous) {
      problems.set(entry.path, '与本次批量名称重复')
      if (previous !== entry.path) problems.set(previous, '与本次批量名称重复')
      continue
    }
    seen.set(key, entry.path)
  }
  return problems
}

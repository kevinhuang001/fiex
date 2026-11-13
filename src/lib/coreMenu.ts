import type { ShortcutAction } from './shortcuts'

const coreMenuCommandValues = [
  'open',
  'reveal',
  'preview',
  'info',
  'tags',
  'rename',
  'batchRename',
  'copy',
  'cut',
  'duplicate',
  'trash',
  'newFolder',
  'newFile.empty',
  'newFile.docx',
  'newFile.xlsx',
  'newFile.pptx',
  'restore',
  'delete',
  'emptyTrash',
  'removeFromRecent',
  'copyPath',
  'chooseOpenWith',
] as const

export type CoreMenuCommand = (typeof coreMenuCommandValues)[number]
const coreMenuCommands = new Set<string>(coreMenuCommandValues)

function isCoreMenuCommand(value: unknown): value is CoreMenuCommand {
  return typeof value === 'string' && coreMenuCommands.has(value)
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
}

export interface CoreMenuAction {
  kind: 'coreAction'
  command: CoreMenuCommand
  paths: string[]
  path: string
  directory: string
}

/** Decode only Core-owned menu actions; plugin-specific JSON results pass through. */
export function parseCoreMenuAction(value: unknown): CoreMenuAction | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
  const action = value as Record<string, unknown>
  if (action.kind !== 'coreAction') return null

  const keys = ['kind', 'command', 'paths', 'path', 'directory']
  if (Object.keys(action).some((key) => !keys.includes(key)))
    throw new TypeError('Invalid Core menu action fields')
  if (
    !isCoreMenuCommand(action.command) ||
    !isStringArray(action.paths) ||
    typeof action.path !== 'string' ||
    typeof action.directory !== 'string'
  )
    throw new TypeError('Invalid Core menu action')

  return {
    kind: 'coreAction',
    command: action.command,
    paths: action.paths,
    path: action.path,
    directory: action.directory,
  }
}

/** Shared wire shape for lazily returned menu rows before their owner is known. */
export interface MenuChildItem {
  id: string
  kind: 'action' | 'submenu' | 'separator'
  title: string
  parentId?: string
  icon?: string
  shortcut?: string
  disabled?: boolean
  danger?: boolean
  placement?: 'tail'
  activity?: MenuActivity
}

export interface MenuActivity {
  label: string
  kind: 'sync' | 'transfer'
  quiet: boolean
}

export interface CoreMenuItem extends Omit<MenuChildItem, 'shortcut'> {
  shortcut?: ShortcutAction
}

export interface CoreMenu {
  kind: 'builtin'
  providerId: string
  menuSessionId: string
  items: CoreMenuItem[]
}

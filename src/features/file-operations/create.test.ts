import type { Entry } from '@/lib/types'
import { describe, expect, it, vi } from 'vitest'
import { createActions } from './create'
import type { FileOpsContext, FileOperationServices } from './types'

function createContext(allowedOps = ['write', 'mkdir']) {
  const setError = vi.fn()
  const setQuery = vi.fn()
  const setSelected = vi.fn()
  const beginRename = vi.fn()
  const run = vi.fn(async (_label: string, operation: () => Promise<unknown>) => operation())
  const services: FileOperationServices = {
    fileInfo: vi.fn(async (path) => ({ path, allowedOps: ['rename'] }) as Entry),
    createEntry: vi.fn(async (_path, name) => `/docs/${name}`),
    createFileEntry: vi.fn(async (_path, name) => `/docs/${name}`),
    recycle: vi.fn(),
    deletePermanently: vi.fn(),
    emptyTrash: vi.fn(),
    trashRestoreNeedsFolder: vi.fn(),
    restoreFromTrash: vi.fn(),
    undo: vi.fn(),
    pickDirectory: vi.fn(),
    fileParent: vi.fn(),
    transfer: vi.fn(),
    reportPermissionError: vi.fn(() => false),
  }
  const context = {
    x: {
      path: '/docs',
      query: 'draft',
      entries: [],
      directoryAllowedOps: allowedOps,
      setError,
      setQuery,
      setSelected,
      beginRename,
    },
    run,
    services,
  } as unknown as FileOpsContext
  return {
    actions: createActions(context),
    context,
    services,
    setError,
    setQuery,
    setSelected,
    beginRename,
  }
}

describe('createActions', () => {
  it('creates through injected services and selects the new file before inline rename', async () => {
    const { actions, services, setQuery, setSelected, beginRename } = createContext()

    await actions.createFile('docx')

    expect(services.createFileEntry).toHaveBeenCalledWith('/docs', '未命名文档.docx', 'docx')
    expect(services.fileInfo).toHaveBeenCalledWith('/docs/未命名文档.docx')
    expect(setQuery).toHaveBeenCalledWith('')
    expect(setSelected).toHaveBeenCalledWith(['/docs/未命名文档.docx'])
    expect(beginRename).toHaveBeenCalledWith('/docs/未命名文档.docx', {
      path: '/docs/未命名文档.docx',
      allowedOps: ['rename'],
    })
  })

  it('does not attempt creation when the provider denies write capability', async () => {
    const { actions, services, setError } = createContext([])

    await actions.createFile()

    expect(services.createFileEntry).not.toHaveBeenCalled()
    expect(setError).toHaveBeenCalledWith('此位置不支持新建文件')
  })
})

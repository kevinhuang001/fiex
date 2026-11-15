import { describe, expect, it, vi } from 'vitest'
import type { CoreMenu, CoreMenuItem } from '@/lib/coreMenu'
import type { Entry } from '@/lib/types'
import type { ContextMenuExecutionServices, ContextMenuProps } from '../types'
import { executeCoreMenu } from './executeCoreMenu'

const entry: Entry = {
  path: '/work/Notes.md',
  name: 'Notes.md',
  isDir: false,
  isSymlink: false,
  size: 12,
  modified: 1,
  created: 1,
  extension: 'md',
  mime: 'text/markdown',
  linkTarget: null,
  tags: [],
  sync: null,
}

function fixture() {
  const item: CoreMenuItem = { id: 'open', kind: 'action', title: 'Open' }
  const menu: CoreMenu = {
    kind: 'builtin',
    providerId: 'builtin:open',
    menuSessionId: 'menu-session',
    items: [item],
  }
  const setError = vi.fn()
  const props = {
    menu: { x: 10, y: 20, entry },
    x: {
      entries: [entry],
      path: '/work',
      selected: [entry.path],
      navigateTo: vi.fn(),
      beginRename: vi.fn(),
      setError,
    },
    open: vi.fn(),
  } as unknown as ContextMenuProps
  const services: ContextMenuExecutionServices = {
    progressChannel: vi.fn(() => null),
    quietProgressChannel: vi.fn(() => ({}) as never),
    executeMenu: vi.fn(async () => ({
      kind: 'coreAction',
      command: 'open',
      paths: [entry.path],
      path: entry.path,
      directory: '/work',
    })),
    recentNavigationTarget: vi.fn(async () => ({ parent: '/work', target: entry.path })),
    copyText: vi.fn(async () => {}),
    refreshDirectories: vi.fn(),
    reportFilesystemPermissionError: vi.fn(() => false),
  }
  return { item, menu, props, services, setError }
}

describe('executeCoreMenu', () => {
  it('executes through the provider session and applies the validated Core action', async () => {
    const { item, menu, props, services } = fixture()

    await executeCoreMenu(menu, item, props, services)

    expect(services.executeMenu).toHaveBeenCalledWith('menu-session', 'open', expect.anything())
    expect(props.open).toHaveBeenCalledWith(entry)
    expect(services.refreshDirectories).not.toHaveBeenCalled()
  })

  it('refreshes the current directory when the provider returns no Core action', async () => {
    const { item, menu, props, services } = fixture()
    vi.mocked(services.executeMenu).mockResolvedValue(null)

    await executeCoreMenu(menu, item, props, services)

    expect(services.refreshDirectories).toHaveBeenCalledWith(['/work'])
    expect(props.open).not.toHaveBeenCalled()
  })

  it('surfaces a failed provider execution through the Explorer error state', async () => {
    const { item, menu, props, services, setError } = fixture()
    vi.mocked(services.executeMenu).mockRejectedValue(new Error('permission denied'))

    await executeCoreMenu(menu, item, props, services)

    expect(services.reportFilesystemPermissionError).toHaveBeenCalled()
    expect(setError).toHaveBeenCalledWith('Error: permission denied')
  })
})

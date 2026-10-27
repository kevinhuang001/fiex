import { describe, expect, it, vi, beforeEach } from 'vitest'
import type { ContextItemsProps } from './types'

const { call } = vi.hoisted(() => ({ call: vi.fn().mockResolvedValue(undefined) }))
vi.mock('@/lib/api', () => ({ call }))
vi.mock('@/hooks/useActivity', () => ({ progressChannel: vi.fn() }))
vi.mock('@/i18n', () => ({ t: (key: string) => key }))
import { resolveContextItems } from './items'

function context(selected = ['/a'], path = '/a'): ContextItemsProps {
  return {
    menu: { entry: { path, sync: null }, x: 0, y: 0 },
    platform: 'linux',
    hint: () => '',
    isRemotePath: (value: string) => value.startsWith('remote://'),
    x: { selected, directoryReadOnly: false, connections: [], reload: vi.fn(), setError: vi.fn() },
    run: vi.fn(async (_label, action) => action()),
    openBatchRename: vi.fn(),
    inTrash: false,
  } as unknown as ContextItemsProps
}
const labels = (props: ContextItemsProps) =>
  resolveContextItems(props).items.map((item) => item.label)

beforeEach(() => call.mockClear())

describe('builtin context menu providers', () => {
  it('matches the effective selection, including a click outside the old selection', () => {
    const props = context(['/a', '/b'])
    expect(labels(props)).toContain('批量重命名…')
    expect(labels(props)).not.toContain('重新命名')
    props.menu.entry.path = '/c'
    const result = resolveContextItems(props)
    expect(result.paths).toEqual(['/c'])
    expect(result.items.map((item) => item.label)).toContain('重新命名')
    expect(result.items.map((item) => item.label)).not.toContain('批量重命名…')
    expect(props.x.selected).toEqual(['/a', '/b'])
  })

  it('keeps destructive actions in the tail independently of translated labels', () => {
    const props = context()
    expect(resolveContextItems(props).tail.map((item) => item.label)).toEqual(['移到废纸篓'])
    props.inTrash = true
    expect(resolveContextItems(props).tail.map((item) => item.label)).toEqual([
      '放回原处',
      '彻底删除',
    ])
    expect(resolveContextItems(props).items.some((item) => item.label === '彻底删除')).toBe(false)
  })

  it('keeps read-only commands disabled and tags restricted to macOS', () => {
    const props = context(['/a', '/b'])
    props.x.directoryReadOnly = true
    expect(
      resolveContextItems(props).items.find((item) => item.label === '批量重命名…')?.disabled,
    ).toBe(true)
    expect(labels(props)).not.toContain('标签…')
    props.platform = 'macos'
    expect(labels(props)).toContain('标签…')
  })

  it('invokes the multi-selection rename command through its provider', () => {
    const props = context(['/a', '/b'])
    const item = resolveContextItems(props).items.find((item) => item.label === '批量重命名…')!
    if ('action' in item) item.action()
    expect(props.openBatchRename).toHaveBeenCalledOnce()
  })

  it('does not expose connection operations on local or mixed selections', () => {
    expect(labels(context())).not.toContain('缓存到本地')
    expect(labels(context(['remote://m/a', '/b'], 'remote://m/a'))).not.toContain('缓存到本地')
  })

  it('executes connection cache actions against the captured selection and refreshes', async () => {
    const paths = ['remote://m/a', 'remote://m/b']
    const props = context(paths, paths[0])
    const item = resolveContextItems(props).items.find((item) => item.label === '缓存到本地')!
    if ('action' in item) item.action()
    await vi.waitFor(() =>
      expect(call).toHaveBeenCalledWith('remote_cache', expect.objectContaining({ paths })),
    )
    await vi.waitFor(() => expect(props.x.reload).toHaveBeenCalledOnce())
  })

  it('offers conflict resolution without offering uncache for a conflicted entry', () => {
    const props = context(['remote://m/a'], 'remote://m/a')
    Object.assign(props.menu.entry, { sync: 'conflict', hasCache: true })
    expect(labels(props)).not.toContain('取消缓存')
    expect(labels(props)).toContain('用本地版本覆盖云端')
    expect(labels(props)).toContain('保留本地备份并采用云端版本')
  })
})

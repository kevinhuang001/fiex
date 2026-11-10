import { describe, expect, it } from 'vitest'
import { buildMoreMenuPlan } from './moreMenu'
import type { CoreMenu } from './useContextMenu'

const menus: CoreMenu[] = [
  {
    kind: 'builtin',
    providerId: 'builtin:duplicate',
    menuSessionId: 'session-copy',
    items: [{ id: 'duplicate', kind: 'action', title: '创建副本' }],
  },
  {
    kind: 'builtin',
    providerId: 'builtin:delete-permanently',
    menuSessionId: 'session-delete',
    items: [{ id: 'delete', kind: 'action', title: '彻底删除', danger: true }],
  },
]

describe('buildMoreMenuPlan', () => {
  it('groups duplicate and permanent deletion, and removes a workspace item', () => {
    const plan = buildMoreMenuPlan(menus, ['/work/report'], ['/work/report'], '/work')

    expect(plan.coreActions.map(({ item }) => item.id)).toEqual(['duplicate', 'delete'])
    expect(plan.coreActions.at(-1)?.item).toMatchObject({ id: 'delete', danger: true })
    expect(plan.workspaceAction).toEqual({ kind: 'remove', paths: ['/work/report'] })
  })

  it('places permanent deletion after the workspace action candidates', () => {
    const plan = buildMoreMenuPlan(menus, ['/work/report'], [], '/work')

    expect(plan.coreActions.map(({ item }) => item.id)).toEqual(['duplicate', 'delete'])
    expect(plan.coreActions.at(-1)?.item.id).toBe('delete')
    expect(plan.workspaceAction).toEqual({ kind: 'add', paths: ['/work/report'] })
  })

  it('adds only selected items that are not already in the workspace', () => {
    const plan = buildMoreMenuPlan(
      menus,
      ['/work/already-added', '/work/new'],
      ['/work/already-added'],
      '/work',
    )

    expect(plan.workspaceAction).toEqual({ kind: 'add', paths: ['/work/new'] })
  })

  it.each([
    'applications://',
    'applications://Utilities',
    'trash://',
    'trash://Folder',
    'recyclebin://',
  ])('does not offer workspace actions in %s', (directory) => {
    const plan = buildMoreMenuPlan(menus, ['/item'], [], directory)
    expect(plan.workspaceAction).toBeNull()
  })
})

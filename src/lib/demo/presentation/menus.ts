// Native menu DTO fixtures for browser tests. Never imported by desktop code.
import { t } from '@/i18n'
import { demoRemoteEntries, demoRemoteRoot } from '../data'
import { state } from '../handlers/shared'
import { handleFiles } from '../handlers/files'
import type { CoreMenu, CoreMenuItem } from '@/components/explorer/context-menu/useContextMenu'
import type { ShortcutAction } from '@/lib/shortcuts'

const sessions = new Map<
  string,
  { paths: string[]; directory: string; actions: Set<string>; created: number }
>()
export function demoBuiltinMenus(a: Record<string, unknown>): CoreMenu[] {
  const paths = a.paths as string[]
  const directory = String(a.directory)
  const recentView = directory === 'recent://' || directory === 'recent:'
  const trashView = directory === 'trash://' || directory === 'recyclebin://'
  const allEntries = [...state.entries, ...demoRemoteEntries]
  const entries = paths.map((path) => allEntries.find((entry) => entry.path === path))
  const remote = paths.length > 0 && paths.every((path) => path.startsWith(demoRemoteRoot))
  const selectedOps = entries.map((entry) => entry?.allowedOps || [])
  const appOps = paths.map((path) =>
    path.startsWith('applications://')
      ? /^applications:\/\/(?:user\/|~)/.test(path)
        ? ['stat', 'remove']
        : ['stat']
      : path.startsWith('trash://') || path.startsWith('recyclebin://')
        ? ['stat', 'remove']
        : [],
  )
  const ops = selectedOps.map((values, index) => {
    const effective = values.length ? values : appOps[index]
    return recentView
      ? effective.filter((operation) => operation === 'stat' || operation === 'read')
      : effective
  })
  const permits = (operation: string) =>
    ops.length > 0 && ops.every((allowed) => allowed.includes(operation))
  const platform = new URLSearchParams(location.search).get('platform') || ''
  // Mirrors Core: an unavailable operation is skipped, never emitted disabled.
  const item = (
    id: string,
    title: string,
    icon: string,
    shortcut?: ShortcutAction,
  ): CoreMenuItem => ({ id, title: t(title), icon, shortcut, disabled: false, kind: 'action' })
  const groups: [string, CoreMenuItem[]][] = [
    [
      'opening',
      paths.length === 0
        ? []
        : [
            item('open', '打开', 'open'),
            ...(paths.length === 1
              ? [
                  ...(permits('read') ? [item('preview', '快速查看', 'eye', 'quickLook')] : []),
                  item('info', '显示简介', 'info', 'info'),
                ]
              : []),
          ],
    ],
  ]
  // Tags and the terminal are local-only in Core, and both need a writable item.
  if (!remote && permits('write') && !['windows', 'linux'].includes(platform))
    groups.push(['tags', [item('tags', '标签…', 'tags')]])
  groups.push(['connection-cache', []])
  if (permits('rename')) {
    if (paths.length === 1)
      groups.push(['rename', [item('rename', '重新命名', 'pencil', 'rename')]])
    if (paths.length > 1)
      groups.push(['batch-rename', [item('batchRename', '批量重命名…', 'pencil')]])
  }
  groups.push([
    'clipboard',
    [
      ...(permits('read') ? [item('copy', '复制', 'copy', 'copy')] : []),
      ...(permits('read') && permits('remove') ? [item('cut', '剪切', 'scissors', 'cut')] : []),
      ...(permits('read') && permits('write')
        ? [item('duplicate', '创建副本', 'copy', 'duplicate')]
        : []),
    ],
  ])
  groups.push(['copy-path', [item('copyPath', '复制路径', 'copy')]])
  groups.push(['trash', []])
  if (remote) {
    const pinned = entries.every((entry) => entry?.pinned)
    const cached = entries.every((entry) => entry?.hasCache)
    const pin = item(
      pinned ? 'remote.unpin' : 'remote.pin',
      pinned ? '取消保留在本地' : '始终保留在本地',
      pinned ? 'pin-off' : 'cloud-download',
    )
    if (!pinned) pin.activity = { label: t('触发同步'), kind: 'sync', quiet: true }
    const remoteItems = [pin]
    if (!cached)
      remoteItems.push({
        ...item('remote.cache', '缓存到本地', 'cloud-download'),
        activity: { label: t('正在缓存…'), kind: 'transfer', quiet: false },
      })
    else if (
      !entries.some(
        (entry) => entry?.pinned || entry?.sync === 'conflict' || entry?.sync === 'pending',
      )
    )
      remoteItems.push(item('remote.uncache', '取消缓存', 'cloud-off'))
    if (entries.some((entry) => entry?.sync === 'conflict'))
      remoteItems.push(
        item('remote.keepLocal', '用本地版本覆盖云端', 'cloud-check'),
        item('remote.keepRemote', '保留本地备份并采用云端版本', 'cloud-check'),
      )
    const cacheGroup = groups.find(([id]) => id === 'connection-cache')
    if (cacheGroup) cacheGroup[1] = remoteItems
  }
  const trashGroup = groups.find(([id]) => id === 'trash')
  if (trashGroup) {
    const canRemove = permits('remove')
    trashGroup[1] = (
      trashView
        ? paths.length === 0
          ? [item('emptyTrash', '清空废纸篓', 'trash')]
          : [
              item('restore', '放回原处', 'restore'),
              ...(canRemove ? [item('delete', '彻底删除', 'trash')] : []),
            ]
        : [
            ...(permits('trash') ? [item('trash', '移到废纸篓', 'trash', 'trash')] : []),
            ...(canRemove ? [item('delete', '彻底删除', 'trash')] : []),
          ]
    ).map((item) => ({ ...item, placement: 'tail', danger: item.id !== 'restore' }))
  }
  for (const [id, session] of sessions)
    if (Date.now() - session.created >= 90000) sessions.delete(id)
  const providerForAction: Record<string, string> = {
    open: 'builtin:open',
    preview: 'builtin:preview',
    info: 'builtin:info',
    tags: 'builtin:tags',
    rename: 'builtin:rename',
    batchRename: 'builtin:batch-rename',
    copy: 'builtin:copy',
    cut: 'builtin:cut',
    duplicate: 'builtin:duplicate',
    copyPath: 'builtin:copy-path',
    'remote.pin': 'builtin:pin',
    'remote.unpin': 'builtin:unpin',
    'remote.cache': 'builtin:cache',
    'remote.uncache': 'builtin:uncache',
    'remote.keepLocal': 'builtin:conflict-resolution',
    'remote.keepRemote': 'builtin:conflict-resolution',
    trash: 'builtin:move-to-trash',
    delete: 'builtin:delete-permanently',
    restore: 'builtin:restore',
    emptyTrash: 'builtin:empty-trash',
  }
  const menus: CoreMenu[] = []
  const addMenu = (providerId: string, items: CoreMenuItem[]) => {
    const menuSessionId = 'builtin-demo-' + crypto.randomUUID()
    sessions.set(menuSessionId, {
      paths: [...paths],
      directory,
      actions: new Set(items.filter((item) => !item.disabled).map((item) => item.id)),
      created: Date.now(),
    })
    menus.push({ kind: 'builtin', providerId, menuSessionId, items })
  }
  for (const [, items] of groups) {
    const conflicts = items.filter(
      (item) => item.id === 'remote.keepLocal' || item.id === 'remote.keepRemote',
    )
    if (conflicts.length) addMenu('builtin:conflict-resolution', conflicts)
    for (const current of items) {
      if (conflicts.includes(current)) continue
      const providerId = providerForAction[current.id]
      if (!providerId) throw new Error(`Unknown demo Core menu action: ${current.id}`)
      addMenu(providerId, [current])
    }
  }
  return menus
}
export async function executeDemoBuiltin(a: Record<string, unknown>) {
  const id = String(a.menuSessionId || '')
  if (!id.startsWith('builtin-demo-')) return undefined
  const session = sessions.get(id)
  sessions.delete(id)
  const command = String(a.itemId)
  if (!session || Date.now() - session.created >= 90000 || !session.actions.has(command))
    throw new Error('Invalid or already used menu session')
  if (!command.startsWith('remote.'))
    return {
      kind: 'coreAction',
      command,
      paths: session.paths,
      path: session.paths[0],
      directory: session.directory,
    }
  if (command === 'remote.cache')
    await handleFiles('remote_cache', { paths: session.paths, onProgress: a.onProgress })
  for (const entry of demoRemoteEntries.filter((entry) => session.paths.includes(entry.path))) {
    if (command === 'remote.cache') {
      entry.hasCache = true
      entry.sync = 'synced'
    }
    if (command === 'remote.pin') {
      entry.pinned = true
      entry.hasCache = true
      entry.sync = 'pinned'
    }
    if (command === 'remote.unpin') {
      entry.pinned = false
      entry.sync = entry.hasCache ? 'synced' : 'cloud'
    }
    if (command === 'remote.uncache') {
      entry.hasCache = false
      entry.uploaded = false
      entry.sync = 'cloud'
    }
    if (command.startsWith('remote.keep') && entry.sync === 'conflict')
      entry.sync = entry.pinned ? 'pinned' : 'synced'
  }
  return { handled: true }
}

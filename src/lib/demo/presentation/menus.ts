// Native menu DTO fixtures for browser tests. Never imported by desktop code.
import { t } from '@/i18n'
import { demoRemoteEntries, demoRemoteRoot, root } from '../data'
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
  const allEntries = [...state.entries, ...demoRemoteEntries]
  const entries = paths.map((path) => allEntries.find((entry) => entry.path === path))
  const remote = paths.every((path) => path.startsWith(demoRemoteRoot))
  const ro = remote || allEntries.find((entry) => entry.path === directory)?.readonly || false
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
      [
        item('open', '打开', 'open'),
        ...(paths.length === 1
          ? [
              item('preview', '快速查看', 'eye', 'quickLook'),
              item('info', '显示简介', 'info', 'info'),
            ]
          : []),
      ],
    ],
  ]
  // Tags and the terminal are local-only in Core, and both need a writable item.
  if (!remote && !ro && !['windows', 'linux'].includes(platform))
    groups.push(['tags', [item('tags', '标签…', 'tags')]])
  groups.push(['connection-cache', []])
  if (!ro) {
    if (paths.length === 1)
      groups.push(['rename', [item('rename', '重新命名', 'pencil', 'rename')]])
    if (paths.length > 1)
      groups.push(['batch-rename', [item('batchRename', '批量重命名…', 'pencil')]])
  }
  groups.push([
    'clipboard',
    [
      item('copy', '复制', 'copy', 'copy'),
      ...(ro
        ? []
        : [
            item('cut', '剪切', 'scissors', 'cut'),
            item('duplicate', '创建副本', 'copy', 'duplicate'),
          ]),
    ],
  ])
  if (!remote && !ro) groups.push(['terminal', [item('terminal', 'Open in Terminal', 'terminal')]])
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
  if (trashGroup)
    trashGroup[1] = (
      directory.startsWith(root + '/.Trash')
        ? [
            item('restore', '放回原处', 'restore'),
            ...(ro ? [] : [item('delete', '彻底删除', 'trash')]),
          ]
        : ro
          ? []
          : [item('trash', '移到废纸篓', 'trash', 'trash')]
    ).map((item) => ({ ...item, placement: 'tail', danger: item.id !== 'restore' }))
  for (const [id, session] of sessions)
    if (Date.now() - session.created >= 90000) sessions.delete(id)
  return groups
    .filter(([, items]) => items.length)
    .map(([id, items]) => {
      const menuSessionId = 'builtin-demo-' + crypto.randomUUID()
      sessions.set(menuSessionId, {
        paths: [...paths],
        directory,
        actions: new Set(items.filter((item) => !item.disabled).map((item) => item.id)),
        created: Date.now(),
      })
      return { kind: 'builtin', providerId: 'builtin:' + id, menuSessionId, items }
    })
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

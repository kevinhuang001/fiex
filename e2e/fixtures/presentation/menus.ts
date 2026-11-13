// Native menu DTO fixtures for browser tests. Never imported by desktop code.
import { t } from '@/i18n'
import { fixtureRemoteEntries, fixtureRemoteRoot } from '../data'
import { state } from '../handlers/shared'
import { handleFiles } from '../handlers/files'
import type { CoreMenu, CoreMenuItem } from '@/lib/coreMenu'
import type { ShortcutAction } from '@/lib/shortcuts'

const sessions = new Map<
  string,
  {
    paths: string[]
    directory: string
    actions: Set<string>
    submenus: Set<string>
    children: Map<string, CoreMenuItem[]>
    created: number
  }
>()
export function fixtureBuiltinMenus(a: Record<string, unknown>): CoreMenu[] {
  const paths = a.paths as string[]
  const directory = String(a.directory)
  const recentView = directory === 'recent://' || directory === 'recent:'
  const trashView = directory === 'trash://' || directory === 'recyclebin://'
  const allEntries = [...state.entries, ...fixtureRemoteEntries]
  const entries = paths.map((path) => allEntries.find((entry) => entry.path === path))
  const remote = paths.length > 0 && paths.every((path) => path.startsWith(fixtureRemoteRoot))
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
  const canCopySources =
    entries.length === paths.length &&
    entries.every((entry, index) => ops[index]?.includes(entry?.isDir ? 'list' : 'read'))
  const canCutSources = canCopySources && permits('remove')
  const destinationOps = allEntries.find((entry) => entry.path === directory)?.allowedOps || []
  const needsDirectory = entries.some((entry) => entry?.isDir)
  const needsFile = entries.some((entry) => !entry?.isDir)
  const canDuplicate =
    canCopySources &&
    (!needsDirectory || destinationOps.includes('mkdir')) &&
    (!needsFile || destinationOps.includes('write'))
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
                  ...(permits('read') &&
                  !entries[0]?.isDir &&
                  !trashView &&
                  !paths[0].startsWith('applications://')
                    ? [item('openWith-choose', '打开方式', 'application-search')]
                    : []),
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
      ...(canCopySources ? [item('copy', '复制', 'copy', 'copy')] : []),
      ...(canCutSources ? [item('cut', '剪切', 'scissors', 'cut')] : []),
      ...(canDuplicate ? [item('duplicate', '创建副本', 'copy', 'duplicate')] : []),
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
              ...(canRemove
                ? [item('delete', paths.length > 1 ? '彻底删除选中项' : '彻底删除', 'trash')]
                : []),
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
    'openWith-choose': 'builtin:open-with',
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
  const addMenu = (
    providerId: string,
    items: CoreMenuItem[],
    children = new Map<string, CoreMenuItem[]>(),
  ) => {
    const menuSessionId = 'builtin-demo-' + crypto.randomUUID()
    const submenus = new Set(items.filter((item) => item.kind === 'submenu').map((item) => item.id))
    sessions.set(menuSessionId, {
      paths: [...paths],
      directory,
      actions: new Set(
        items.filter((item) => item.kind === 'action' && !item.disabled).map((item) => item.id),
      ),
      submenus,
      children,
      created: Date.now(),
    })
    menus.push({ kind: 'builtin', providerId, menuSessionId, items })
  }
  if (paths.length === 0 && destinationOps.includes('write')) {
    const root = { ...item('newFile', '新建文件', 'file-plus'), kind: 'submenu' as const }
    const empty = item('newFile.empty', '空白文件 (.txt)', 'file-text')
    const office = { ...item('newFile.office', 'Office', 'application'), kind: 'submenu' as const }
    const docx = item('newFile.docx', 'Word 文档 (.docx)', 'file-text')
    const xlsx = item('newFile.xlsx', 'Excel 工作簿 (.xlsx)', 'file-spreadsheet')
    const pptx = item('newFile.pptx', 'PowerPoint 演示文稿 (.pptx)', 'presentation')
    addMenu(
      'builtin:new-file',
      [root],
      new Map([
        [
          'newFile',
          [
            { ...empty, parentId: 'newFile' },
            { ...office, parentId: 'newFile' },
          ],
        ],
        [
          'newFile.office',
          [
            { ...docx, parentId: 'newFile.office' },
            { ...xlsx, parentId: 'newFile.office' },
            { ...pptx, parentId: 'newFile.office' },
          ],
        ],
      ]),
    )
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
export async function executeFixtureBuiltin(a: Record<string, unknown>) {
  const id = String(a.menuSessionId || '')
  if (!id.startsWith('builtin-demo-')) return undefined
  const session = sessions.get(id)
  sessions.delete(id)
  const command = String(a.itemId)
  if (!session || Date.now() - session.created >= 90000 || !session.actions.has(command))
    throw new Error('Invalid or already used menu session')
  if (command === 'openWith-choose')
    return {
      kind: 'coreAction',
      command: 'chooseOpenWith',
      paths: session.paths,
      path: session.paths[0] || '',
      directory: session.directory,
    }
  if (command.startsWith('openWith-')) return { handled: true }
  if (!command.startsWith('remote.'))
    return {
      kind: 'coreAction',
      command,
      paths: session.paths,
      path: session.paths[0] || '',
      directory: session.directory,
    }
  if (command === 'remote.cache')
    await handleFiles('remote_cache', { paths: session.paths, onProgress: a.onProgress })
  for (const entry of fixtureRemoteEntries.filter((entry) => session.paths.includes(entry.path))) {
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

export function fixtureBuiltinChildren(a: Record<string, unknown>) {
  const sessionId = String(a.menuSessionId || '')
  const parentId = String(a.parentId || '')
  const session = sessions.get(sessionId)
  if (!sessionId.startsWith('builtin-demo-')) return undefined
  if (!session || Date.now() - session.created >= 90000 || !session.submenus.has(parentId))
    throw new Error('Invalid Core menu session or submenu')
  const items = session.children.get(parentId) || []
  for (const item of items) {
    if (item.kind === 'action') session.actions.add(item.id)
    if (item.kind === 'submenu') session.submenus.add(item.id)
  }
  return { items: items.map((item) => ({ ...item, parentId })) }
}

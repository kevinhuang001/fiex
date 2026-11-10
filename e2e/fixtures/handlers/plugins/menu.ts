import {
  fixtureBuiltinChildren,
  fixtureBuiltinMenus,
  executeFixtureBuiltin,
} from '../../presentation/menus'
import { fixturePlugins } from '../../plugins'
import { requestFixturePermission } from '../../permission'
import { basename, parent } from '@/lib/files'
import { handleFiles } from '../files'
import { notHandled } from '../shared'

interface FixtureMenuItem {
  id: string
  title: string
  kind: string
  parentId?: string
}
const fixtureMenuSessions = new Map<
  string,
  {
    pluginId: string
    actions: Set<string>
    submenus: Set<string>
    items: FixtureMenuItem[]
    paths: string[]
    created: number
  }
>()

function resolveFixtureMenu(a: Record<string, unknown>) {
  const pluginId = String(a.pluginId || '')
  const plugin = fixturePlugins.find((p) => p.id === pluginId)
  if (!plugin?.enabled || !plugin.extensions?.contextMenus) {
    throw new Error('Context menu provider is not active')
  }
  const selection = Array.isArray(a.paths)
    ? a.paths.filter((v): v is string => typeof v === 'string')
    : []
  if (selection.length === 0) throw new Error('No selected resources')
  const selectionNames = selection.map((path) => path.split(/[\\/]/).pop() || '')
  const entries = [
    ...(pluginId === 'org.test.fixture'
      ? [
          { id: 'fixture.root', title: '测试插件', kind: 'submenu' },
          { id: 'fixture.panel', title: '信息面板', kind: 'action', parentId: 'fixture.root' },
          { id: 'fixture.action', title: '检查文件', kind: 'action', parentId: 'fixture.root' },
        ]
      : []),
    ...(pluginId === 'org.example.checksum' && a.isDirectory !== true
      ? [{ id: 'checksum.calculate', title: '查看校验和', kind: 'action' }]
      : []),
    ...(pluginId === 'org.fiex.office-preview' &&
    selection.length === 1 &&
    /\.(docx|xlsx|pptx)$/i.test(selectionNames[0])
      ? [
          { id: 'office.menu', title: 'Office', kind: 'submenu' },
          {
            id: 'office.convertToPdf',
            title: '转换为 PDF',
            kind: 'action',
            parentId: 'office.menu',
          },
        ]
      : []),
    ...(pluginId === 'org.fiex.archive'
      ? selection.length === 1 &&
        /\.(zip|rar|7z|tar|gz|tgz|bz2|tbz2|xz|txz)$/i.test(selectionNames[0])
        ? [{ id: 'archive.extract', title: '解压缩', kind: 'action' }]
        : [
            { id: 'archive.compress', title: '压缩', kind: 'submenu' },
            {
              id: 'archive.advanced',
              title: '高级压缩…',
              kind: 'action',
              parentId: 'archive.compress',
            },
          ]
      : []),
    ...(pluginId === 'org.fiex.gpg'
      ? [
          { id: 'gpg.menu', title: 'GPG', kind: 'submenu' },
          { id: 'gpg.encrypt', title: '加密…', kind: 'action', parentId: 'gpg.menu' },
          { id: 'gpg.decrypt', title: '解密…', kind: 'action', parentId: 'gpg.menu' },
          { id: 'gpg.sign', title: '签名…', kind: 'action', parentId: 'gpg.menu' },
          ...(selection.length === 1
            ? [{ id: 'gpg.verify', title: '验证签名…', kind: 'action', parentId: 'gpg.menu' }]
            : []),
        ]
      : []),
  ]
  const menuSessionId = crypto.randomUUID()
  fixtureMenuSessions.set(menuSessionId, {
    pluginId,
    actions: new Set(
      entries
        .filter((entry) => entry.kind === 'action' && !('parentId' in entry))
        .map((entry) => entry.id),
    ),
    submenus: new Set(
      entries
        .filter((entry) => entry.kind === 'submenu' && !('parentId' in entry))
        .map((entry) => entry.id),
    ),
    items: entries,
    paths: selection,
    created: Date.now(),
  })
  return { pluginId, menuSessionId, items: entries.filter((entry) => !('parentId' in entry)) }
}

export async function handlePluginMenu(
  command: string,
  a: Record<string, unknown>,
): Promise<unknown> {
  switch (command) {
    case 'plugin_context_menu_resolve': {
      // Core and plugin rows resolve separately so the top-level menu never
      // waits for plugin IPC; "all" keeps the one-batch contract for callers
      // that still ask for it.
      const source = String(a.source || 'all')
      const plugins = fixturePlugins
        .filter((plugin) => plugin.enabled && !!plugin.extensions?.contextMenus)
        .flatMap((plugin) => {
          try {
            return [{ ...resolveFixtureMenu({ ...a, pluginId: plugin.id }), kind: 'plugin' }]
          } catch {
            return []
          }
        })
      if (source === 'plugins') return plugins
      const builtins = fixtureBuiltinMenus(a)
      if (source === 'core') return builtins
      return [...builtins, ...plugins]
    }
    case 'plugin_context_menu_children': {
      const builtin = fixtureBuiltinChildren(a)
      if (builtin) return builtin
      const session = fixtureMenuSessions.get(String(a.menuSessionId || ''))
      const parentId = String(a.parentId || '')
      if (!session || Date.now() - session.created >= 90000 || !session.submenus.has(parentId))
        throw new Error('Invalid menu session or submenu')
      const children = session.items.filter((item) => item.parentId === parentId)
      for (const item of children) {
        if (item.kind === 'action') session.actions.add(item.id)
        if (item.kind === 'submenu') session.submenus.add(item.id)
      }
      return { items: children }
    }
    case 'plugin_context_menu_execute': {
      const virtual = String(a.menuSessionId || '')
      if (virtual.startsWith('virtual:')) {
        const paths = JSON.parse(virtual.slice('virtual:'.length)) as string[]
        return {
          kind: 'coreAction',
          command: String(a.itemId || ''),
          paths,
          path: paths[0] || '',
        }
      }
      const builtin = await executeFixtureBuiltin(a)
      if (builtin) return builtin
      const menuSessionId = String(a.menuSessionId || '')
      const session = fixtureMenuSessions.get(menuSessionId)
      fixtureMenuSessions.delete(menuSessionId)
      const itemId = String(a.itemId || '')
      if (!session || Date.now() - session.created >= 90000 || !session.actions.has(itemId)) {
        throw new Error('Invalid or already used menu session')
      }
      // The checksum action reads file contents. Mirror Core: if the user
      // revoked files.read, the action waits for the Core-owned prompt before
      // it can continue. This is the "during plugin use" re-authorization case.
      if (itemId === 'office.convertToPdf') {
        const source = session.paths[0]
        if (!source) throw new Error('Office conversion requires one selected file')
        const filename = basename(source).replace(/\.[^.]+$/, '') + '.pdf'
        await handleFiles('create_file_entry', { path: parent(source), name: filename })
      }
      if (itemId === 'checksum.calculate') {
        const plugin = fixturePlugins.find((entry) => entry.id === session.pluginId)
        if (plugin && !plugin.grantedPermissions.includes('files.read')) {
          const allow = await requestFixturePermission({
            pluginId: plugin.id,
            pluginName: plugin.name,
            permission: 'files.read',
            kind: 'permission',
          })
          if (!allow) throw new Error('PermissionDenied: user denied files.read')
        }
      }
      const panelId = itemId
      if (
        itemId === 'fixture.panel' ||
        itemId === 'archive.advanced' ||
        itemId.startsWith('gpg.')
      ) {
        const title =
          itemId === 'fixture.panel'
            ? '信息面板'
            : itemId === 'archive.advanced'
              ? '高级压缩'
              : itemId === 'gpg.encrypt'
                ? 'GPG 加密'
                : itemId === 'gpg.decrypt'
                  ? 'GPG 解密'
                  : itemId === 'gpg.sign'
                    ? 'GPG 签名'
                    : '验证签名'
        return {
          kind: 'panel',
          assetPath: 'demo-plugin-panel',
          panelId,
          title,
          size:
            itemId === 'fixture.panel' ? { width: 620, height: 560 } : { width: 520, height: 420 },
        }
      }
      return { ok: true }
    }
    default:
      return notHandled
  }
}

import { describe, expect, it } from 'vitest'
import {
  parseConnectionMenus,
  parseConnectionOpenResult,
  parseDirectoryViewUpdate,
  parseEntryDecorations,
  parseInfoFieldDelta,
  parseJsonValue,
  serializeIpcJson,
  parsePluginMenuResult,
  parseRemoteConnectionConfig,
  parseSearchBatch,
  parseStateGetResult,
} from './ipcValidation'

describe('IPC boundary validation', () => {
  it('accepts JSON data and rejects values the Rust JSON wire format cannot represent', () => {
    expect(parseJsonValue({ name: 'report', tags: ['work', null] })).toEqual({
      name: 'report',
      tags: ['work', null],
    })
    expect(() => parseJsonValue(Number.NaN)).toThrow('Invalid JSON IPC value')
    expect(() => parseJsonValue(new Date())).toThrow('Invalid JSON IPC object')
    const cyclic: Record<string, unknown> = {}
    cyclic.self = cyclic
    expect(() => parseJsonValue(cyclic)).toThrow('Invalid JSON IPC value')
  })

  it('normalizes command arguments the same way as the JSON wire format', () => {
    expect(serializeIpcJson({ optional: undefined, nested: [undefined] })).toEqual({
      nested: [null],
    })
    expect(() => serializeIpcJson(undefined)).toThrow('Invalid JSON IPC value')
  })

  it('validates both connection navigation variants', () => {
    expect(parseConnectionOpenResult({ kind: 'vfs', path: 'fiex://mount/home' })).toEqual({
      kind: 'vfs',
      path: 'fiex://mount/home',
    })
    expect(
      parseConnectionOpenResult({
        kind: 'pluginView',
        pluginId: 'mail',
        connectionId: 'account-1',
        assetPath: '/plugins/mail/index.html',
        state: { folder: 'inbox' },
      }),
    ).toMatchObject({ kind: 'pluginView', pluginId: 'mail', state: { folder: 'inbox' } })
    expect(() => parseConnectionOpenResult({ kind: 'vfs', path: 42 })).toThrow(
      'Invalid connection navigation response',
    )
  })

  it('normalizes absent optional decoration fields serialized as null by Rust', () => {
    expect(
      parseEntryDecorations([
        {
          path: 'applications://Utilities/Editor.app',
          nativeBadges: [],
          label: null,
          description: null,
          fields: {},
          icon: null,
          iconPath: null,
          providers: [],
        },
      ]),
    ).toEqual([
      {
        path: 'applications://Utilities/Editor.app',
        nativeBadges: [],
        fields: {},
        providers: [],
      },
    ])
  })

  it('validates remote connection settings as scalar JSON values', () => {
    expect(
      parseRemoteConnectionConfig({
        id: 'connection-1',
        name: 'SFTP',
        kind: 'sftp',
        pluginId: null,
        settings: { host: 'example.test', port: 22, enabled: true },
      }),
    ).toMatchObject({ settings: { host: 'example.test', port: 22, enabled: true } })
    expect(() =>
      parseRemoteConnectionConfig({
        id: 'connection-1',
        name: 'SFTP',
        kind: 'sftp',
        pluginId: null,
        settings: { nested: { invalid: true } },
      }),
    ).toThrow('Invalid remote connection config')
  })

  it('rejects unknown remote connection config fields', () => {
    expect(() =>
      parseRemoteConnectionConfig({
        id: null,
        name: 'SFTP',
        kind: 'sftp',
        pluginId: null,
        settings: {},
        secretKeys: ['password'],
      }),
    ).toThrow('Invalid remote connection config')
  })

  it('decodes stored values according to their namespace and key', () => {
    expect(parseStateGetResult({ namespace: 'layout', key: 'columns.hash' }, { size: 20 })).toEqual(
      { name: 260, modified: 155, size: 60, kind: 108 },
    )
    expect(
      parseStateGetResult(
        { namespace: 'picker', key: 'sort' },
        { key: 'modified', direction: 'desc' },
      ),
    ).toEqual({ key: 'modified', direction: 'desc' })
    expect(() =>
      parseStateGetResult(
        { namespace: 'picker', key: 'sort' },
        { key: 'unknown', direction: 'desc' },
      ),
    ).toThrow('Invalid saved picker sort')
    expect(() =>
      parseStateGetResult({ namespace: 'explorer', key: 'locations' }, [{ path: '/home' }]),
    ).toThrow('Invalid saved location')
  })

  it('rejects malformed plugin menus before rendering them', () => {
    expect(
      parsePluginMenuResult(
        [
          {
            kind: 'plugin',
            providerId: 'archive.contextMenu',
            pluginId: 'archive',
            menuSessionId: 'session-1',
            items: [{ id: 'extract', title: 'Extract', kind: 'action' }],
          },
        ],
        'plugins',
      ),
    ).toHaveLength(1)
    expect(() =>
      parsePluginMenuResult(
        [{ pluginId: 'archive', menuSessionId: 'session-1', items: [{ id: 'bad id' }] }],
        'plugins',
      ),
    ).toThrow('Invalid context menu')
  })

  it('validates connection menu items and streamed search entries', () => {
    const menus = parseConnectionMenus([
      {
        kind: 'plugin',
        providerId: 'plugin:sync',
        pluginId: 'sync',
        menuSessionId: 'session-2',
        items: [
          {
            id: 'sync.now',
            kind: 'action',
            title: 'Sync now',
            activity: { label: 'Syncing', kind: 'sync', quiet: true },
          },
        ],
      },
    ])
    expect(menus).toHaveLength(1)
    expect(menus[0].items[0].activity).toEqual({ label: 'Syncing', kind: 'sync', quiet: true })
    expect(() => parseConnectionMenus([{ kind: 'plugin', items: [] }])).toThrow(
      'Invalid connection menu',
    )
    expect(() =>
      parseConnectionMenus([
        {
          kind: 'plugin',
          providerId: 'plugin:sync',
          pluginId: 'sync',
          menuSessionId: 'session-2',
          items: [
            { id: 'sync.now', kind: 'action', title: 'Sync now', activity: { kind: 'sync' } },
          ],
        },
      ]),
    ).toThrow('Invalid connection menu activity')

    const entry = {
      path: '/home/report.txt',
      name: 'report.txt',
      isDir: false,
      isSymlink: false,
      size: 12,
      modified: 1,
      created: 1,
      extension: 'txt',
      mime: 'text/plain',
      linkTarget: null,
      allowedOps: ['stat', 'read'],
      tags: [],
      hasCache: false,
      pinned: false,
      uploaded: false,
      sync: null,
    }
    expect(
      parseSearchBatch({
        entries: [entry],
        done: true,
        elapsedMs: 2,
        truncated: false,
        skipped: 0,
      }),
    ).toMatchObject({ entries: [entry], done: true })
    expect(() =>
      parseSearchBatch({
        entries: [{ ...entry, size: 'large' }],
        done: true,
        elapsedMs: 2,
        truncated: false,
        skipped: 0,
      }),
    ).toThrow('Invalid file entry from IPC')
  })

  it('validates file info updates and directory stream snapshots', () => {
    expect(
      parseInfoFieldDelta({
        sessionId: 'info-1',
        revision: 3,
        updates: [
          {
            providerId: 'core',
            subsectionId: 'general',
            fieldId: 'size',
            status: 'ready',
            value: '12 bytes',
          },
        ],
      }).updates,
    ).toHaveLength(1)
    expect(() =>
      parseInfoFieldDelta({ sessionId: 'info-1', revision: 3, updates: [{ status: 'ready' }] }),
    ).toThrow('Invalid file info update')

    const entry = {
      path: '/home/report.txt',
      name: 'report.txt',
      isDir: false,
      isSymlink: false,
      size: 12,
      modified: 1,
      created: 1,
      extension: 'txt',
      mime: 'text/plain',
      linkTarget: null,
      allowedOps: ['stat', 'read'],
      tags: [],
      hasCache: false,
      pinned: false,
      uploaded: false,
      sync: null,
    }
    expect(
      parseDirectoryViewUpdate({
        listing: {
          path: '/home',
          entries: [entry],
          allowedOps: ['list'],
          warnings: [],
          virtualVolume: false,
        },
        decorations: [{ path: entry.path, label: 'Report' }],
        complete: true,
        presentationPending: false,
        revision: 5,
      }).listing?.path,
    ).toBe('/home')
  })
})

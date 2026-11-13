import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { parseCoreMenuItems, parsePluginMenuItems, parsePluginMenuResult } from './contextMenus'

describe('context menu IPC decoding', () => {
  it('constructs explicit Core and plugin menu DTOs and rejects malformed fields', () => {
    expect(
      parsePluginMenuResult(
        [
          {
            kind: 'builtin',
            providerId: 'core.file.open',
            menuSessionId: 'core-session',
            items: [
              {
                id: 'open',
                kind: 'action',
                title: 'Open',
                shortcut: 'open',
                disabled: false,
                activity: { label: 'Opening', kind: 'transfer', quiet: true },
              },
            ],
          },
        ],
        'core',
      ),
    ).toEqual([
      {
        kind: 'builtin',
        providerId: 'core.file.open',
        menuSessionId: 'core-session',
        items: [
          {
            id: 'open',
            kind: 'action',
            title: 'Open',
            shortcut: 'open',
            disabled: false,
            activity: { label: 'Opening', kind: 'transfer', quiet: true },
          },
        ],
      },
    ])

    expect(
      parsePluginMenuResult(
        [
          {
            kind: 'plugin',
            providerId: 'archive.contextMenu',
            pluginId: 'archive',
            menuSessionId: 'plugin-session',
            items: [{ id: 'extract', title: 'Extract', kind: 'action', icon: 'archive' }],
          },
        ],
        'plugins',
      ),
    ).toEqual([
      {
        kind: 'plugin',
        providerId: 'archive.contextMenu',
        pluginId: 'archive',
        menuSessionId: 'plugin-session',
        items: [{ id: 'extract', title: 'Extract', kind: 'action', icon: 'archive' }],
      },
    ])

    expect(() =>
      parsePluginMenuResult(
        [
          {
            kind: 'builtin',
            providerId: 'core.file.open',
            menuSessionId: 'core-session',
            items: [{ id: 'open', kind: 'action', title: 'Open', shortcut: 'unknown' }],
          },
        ],
        'core',
      ),
    ).toThrow('Invalid Core context menu item')
    expect(() =>
      parsePluginMenuResult(
        [
          {
            kind: 'plugin',
            providerId: 'archive.contextMenu',
            pluginId: 'archive',
            menuSessionId: 'plugin-session',
            items: [{ id: 'extract', title: 'Extract', kind: 'action', unexpected: true }],
          },
        ],
        'plugins',
      ),
    ).toThrow('Invalid context menu')
  })

  it('decodes lazy submenu pages without requiring their parent in the page', () => {
    expect(
      parseCoreMenuItems([
        {
          id: 'copyPath',
          kind: 'action',
          title: 'Copy path',
          parentId: 'more',
          activity: { label: 'Copying', kind: 'transfer', quiet: false },
        },
      ]),
    ).toEqual([
      {
        id: 'copyPath',
        kind: 'action',
        title: 'Copy path',
        parentId: 'more',
        activity: { label: 'Copying', kind: 'transfer', quiet: false },
      },
    ])
    expect(() =>
      parseCoreMenuItems([
        { id: 'invalid', kind: 'action', title: 'Invalid', shortcut: 'invalid' },
      ]),
    ).toThrow('Invalid Core context menu item')

    expect(
      parsePluginMenuItems([
        { id: 'extract', title: 'Extract', kind: 'action', parentId: 'archive' },
      ]),
    ).toEqual([{ id: 'extract', title: 'Extract', kind: 'action', parentId: 'archive' }])
    expect(() =>
      parsePluginMenuItems([{ id: 'extract', title: 'Extract', kind: 'action', extra: true }]),
    ).toThrow('Invalid plugin context menu item')
  })

  it('validates the lazy submenu IPC envelope and shared item DTO', () => {
    expect(
      decodeDynamicIpcResult(
        'plugin_context_menu_children',
        {},
        {
          items: [
            {
              id: 'openRemote',
              kind: 'action',
              title: 'Open remote',
              parentId: 'remote',
              shortcut: 'providerDefined',
            },
          ],
        },
      ),
    ).toEqual({
      items: [
        {
          id: 'openRemote',
          kind: 'action',
          title: 'Open remote',
          parentId: 'remote',
          shortcut: 'providerDefined',
        },
      ],
    })
    expect(() =>
      decodeDynamicIpcResult('plugin_context_menu_children', {}, { items: [], extra: true }),
    ).toThrow('Invalid context submenu response')
    expect(() =>
      decodeDynamicIpcResult(
        'plugin_context_menu_children',
        {},
        {
          items: [{ id: 'bad', kind: 'action', title: 'Bad', nested: [] }],
        },
      ),
    ).toThrow('Invalid context submenu item')
  })
})

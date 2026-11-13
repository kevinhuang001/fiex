import { describe, expect, it } from 'vitest'
import { parsePluginMenuResult } from './menus'

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
})

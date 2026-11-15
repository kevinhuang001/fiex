import { afterEach, expect, it, vi } from 'vitest'
import { parseConnectionOpenResult, parseSearchBatch } from './entries'
import { parsePluginMenuResult } from './contextMenus'
import { parseInstallPreview } from './installation'
import { parseMarketplacePlugins } from './marketplace'
import { parseOperationResult } from './operations'
import { parsePluginCatalog } from './pluginCatalog'
import { parseRemoteConnectionConfig } from './providers'
import { decodeSimpleIpcResult } from './simpleResults'

afterEach(() => vi.unstubAllGlobals())

it('keeps browser fixture commands compatible with Core response contracts', async () => {
  vi.stubGlobal('location', { search: '?fixture&pluginFixture' })
  vi.stubGlobal('crypto', { randomUUID: () => 'test-uuid' })
  const { mock } = await import('../../../e2e/fixtures')
  const root = '/Users/kevin/Documents'
  expect(() => parsePluginCatalog([])).not.toThrow()
  expect(parsePluginCatalog(await mock('plugins_list', {})).length).toBeGreaterThan(0)
  expect(
    parsePluginMenuResult(
      await mock('plugin_context_menu_resolve', {
        source: 'core',
        directory: root,
        paths: [`${root}/Notes.md`],
      }),
      'core',
    ).length,
  ).toBeGreaterThan(0)
  expect(
    parsePluginMenuResult(
      await mock('plugin_context_menu_resolve', {
        source: 'plugins',
        directory: root,
        paths: [`${root}/Notes.md`],
      }),
      'plugins',
    ).length,
  ).toBeGreaterThan(0)
  await expect(mock('plugin_context_menu_resolve', { directory: root, paths: [] })).rejects.toThrow(
    'Invalid menu source',
  )
  expect(parseMarketplacePlugins(await mock('plugin_marketplace', {})).length).toBeGreaterThan(0)
  expect(parseInstallPreview(await mock('plugin_prepare_local_install', {}), true).id).toBe(
    'org.example.local',
  )
  expect(
    parseRemoteConnectionConfig(await mock('remote_connection_config', { id: 'demo-ssh' })),
  ).toMatchObject({ id: 'demo-ssh', kind: 'ssh' })
  expect(
    parseConnectionOpenResult(await mock('connection_open', { id: 'demo-ssh' })),
  ).toMatchObject({ kind: 'pluginView', connectionId: 'demo-ssh' })
  expect(
    parseSearchBatch(
      await mock('search_files', { root, query: 'notes', hidden: false, ignores: [] }),
    ).entries,
  ).toHaveLength(1)
  const conflict = await mock('transfer', { paths: [`${root}/Notes.md`], destination: root })
  expect(parseOperationResult(conflict).conflicts).toHaveLength(1)
  expect(parseOperationResult(await mock('recycle', { paths: [] })).conflicts).toEqual([])
  expect(
    decodeSimpleIpcResult('local_terminal_open', await mock('local_terminal_open', {})),
  ).toEqual({ value: 'fixture-terminal-test-uuid' })
})

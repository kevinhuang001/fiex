import { describe, expect, it } from 'vitest'
import { isNewerVersion, pluginCategories } from './plugins'

describe('plugin catalog helpers', () => {
  it('offers updates only for newer numeric versions', () => {
    expect(isNewerVersion('0.3.0', '0.2.9')).toBe(true)
    expect(isNewerVersion('1.0.0', '1.0.0')).toBe(false)
    expect(isNewerVersion('0.2.0', '0.3.0')).toBe(false)
    expect(isNewerVersion('1.0.1-beta', '1.0.0')).toBe(true)
  })

  it('derives every matching contribution category', () => {
    const categories = pluginCategories(
      {
        info: { match: { extensions: ['txt'] } },
        contextMenus: true,
        connectionContextMenus: true,
        preview: undefined,
      },
      ['make'],
    )
    expect(categories).toEqual(['info', 'contextMenus', 'connections'])
  })

  it('falls back to commands when no contribution category exists', () => {
    expect(pluginCategories({}, ['make'])).toEqual(['commands'])
    expect(pluginCategories({}, [])).toEqual([])
  })
})

import { describe, expect, it } from 'vitest'
import { pluginPreviewId } from './preview'

describe('preview provider helpers', () => {
  it('derives a plugin id only from plugin providers', () => {
    expect(pluginPreviewId('plugin:org.fiex.office-preview')).toBe('org.fiex.office-preview')
    expect(pluginPreviewId('builtin:text')).toBeNull()
    expect(pluginPreviewId('')).toBeNull()
    expect(pluginPreviewId(null)).toBeNull()
    expect(pluginPreviewId(undefined)).toBeNull()
  })
})

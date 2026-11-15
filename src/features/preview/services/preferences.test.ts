import { describe, expect, it } from 'vitest'
import { readPreviewPreferences, type PreviewPreferenceStorage } from './preferences'

function storage(values: Record<string, string>): PreviewPreferenceStorage {
  return { getItem: (key) => values[key] ?? null }
}

describe('preview preferences', () => {
  it('reads string ids and discards invalid list members', () => {
    expect(
      readPreviewPreferences(
        storage({
          'fiex.preview.order': '["plugin:a", 1, "builtin:text"]',
          'fiex.preview.disabled': '["plugin:b", false]',
        }),
      ),
    ).toEqual({ order: ['plugin:a', 'builtin:text'], disabled: ['plugin:b'] })
  })

  it('returns empty preferences when local data is malformed', () => {
    expect(
      readPreviewPreferences(
        storage({
          'fiex.preview.order': '{bad json',
          'fiex.preview.disabled': 'null',
        }),
      ),
    ).toEqual({ order: [], disabled: [] })
  })
})

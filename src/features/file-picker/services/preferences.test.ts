import { describe, expect, it, vi } from 'vitest'
import type { GroupKey, PickerSort, PickerStorage } from '../types'
import {
  persistPickerDirectory,
  persistPickerSort,
  storedDirectory,
  storedGroup,
  storedSort,
} from './preferences'

function storage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial))
  const adapter: PickerStorage = {
    getItem: (key) => values.get(key) || null,
    setItem: (key, value) => void values.set(key, value),
    removeItem: (key) => void values.delete(key),
  }
  return { adapter, values }
}

describe('file-picker preferences', () => {
  it('validates local sort and group settings before using them', () => {
    const local = storage({
      'fiex.picker.sort': JSON.stringify({ key: 'size', direction: 'sideways' }),
      'fiex.picker.group': 'kind',
    })

    expect(storedSort(local.adapter)).toEqual({ key: 'size', direction: 'asc' })
    expect(storedGroup(local.adapter)).toBe('kind')
    local.adapter.setItem('fiex.picker.sort', JSON.stringify({ key: 'unknown' }))
    local.adapter.setItem('fiex.picker.group', 'unknown')
    expect(storedSort(local.adapter)).toEqual({ key: 'name', direction: 'asc' })
    expect(storedGroup(local.adapter)).toBe('none')
  })

  it('uses local fallback values when storage data is malformed', () => {
    const local = storage({
      'fiex.picker.sort': '{not json',
      'fiex.picker.directory': '/remembered',
    })

    expect(storedSort(local.adapter)).toEqual({ key: 'name', direction: 'asc' })
    expect(storedDirectory(local.adapter)).toBe('/remembered')
  })

  it('clears local sort fallbacks after both persistent writes succeed', async () => {
    const sort: PickerSort = { key: 'modified', direction: 'desc' }
    const group: GroupKey = 'size'
    const local = storage({
      'fiex.picker.sort': 'old sort',
      'fiex.picker.group': 'old group',
    })
    const writeState = vi.fn(async () => undefined)

    await persistPickerSort(sort, group, { storage: local.adapter, writeState })

    expect(writeState).toHaveBeenCalledTimes(2)
    expect(local.values.has('fiex.picker.sort')).toBe(false)
    expect(local.values.has('fiex.picker.group')).toBe(false)
  })

  it('preserves sort and group locally if either persistent write fails', async () => {
    const sort: PickerSort = { key: 'size', direction: 'asc' }
    const group: GroupKey = 'modified'
    const local = storage()
    const writeState = vi.fn(async () => {
      throw new Error('offline')
    })

    await persistPickerSort(sort, group, { storage: local.adapter, writeState })

    expect(JSON.parse(local.values.get('fiex.picker.sort') || 'null')).toEqual(sort)
    expect(local.values.get('fiex.picker.group')).toBe(group)
  })

  it('remembers the last directory using the mode-specific key and local fallback', async () => {
    const local = storage({ 'fiex.picker.directory': '/old' })
    const writes: Array<{ key: string; value: unknown }> = []
    const writeState = vi.fn(async (write: { key: string; value: unknown }) => {
      writes.push(write)
    })
    const preferenceServices = { storage: local.adapter, writeState }

    await persistPickerDirectory('/save', 'saveFile', preferenceServices)
    expect(writes[0]).toEqual({ key: 'lastSaveDirectory', value: '/save' })
    expect(local.values.has('fiex.picker.directory')).toBe(false)
    await persistPickerDirectory('/open', 'openFile', {
      ...preferenceServices,
      writeState: async () => {
        throw new Error('offline')
      },
    })
    expect(writes[1]).toBeUndefined()
    expect(local.values.get('fiex.picker.directory')).toBe('/open')
  })
})

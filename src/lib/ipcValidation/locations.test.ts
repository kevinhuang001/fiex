import type { JsonValue } from '../../../sdk/ui/src/index'
import { describe, expect, it } from 'vitest'
import { parseLocations } from './entries'

const location: JsonValue = {
  name: 'External Drive',
  path: '/media/kevin/drive',
  kind: 'disk',
  available: 1024,
  total: 4096,
  removable: true,
  filesystem: 'ext4',
  device: '/dev/sdb1',
  mounted: true,
  usbfs: false,
}

describe('volume location IPC validation', () => {
  it('constructs location DTOs from the Rust serialized fields', () => {
    const decoded = parseLocations([location])

    expect(decoded).toEqual([location])
    expect(decoded).not.toBe(location)
    expect(decoded[0]).not.toBe(location)
  })

  it('accepts an empty location list', () => {
    expect(parseLocations([])).toEqual([])
  })

  it.each([
    [{ ...location, extra: true }],
    [{ ...location, available: -1 }],
    [{ ...location, total: 1.5 }],
    [{ ...location, mounted: 'yes' }],
    [{ ...location, device: null }],
  ] as JsonValue[][])('rejects malformed location data: %j', (value) => {
    expect(() => parseLocations(value)).toThrow()
  })
})

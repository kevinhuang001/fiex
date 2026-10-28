import { t } from '@/i18n'
import { isWindows } from './platform'
import type { Location } from './types'

export function commonPlaces(locations: Location[]): Location[] {
  const result = locations.filter((item) =>
    ['favorite', 'applications', 'recent', 'trash'].includes(item.kind),
  )
  for (const [kind, name, path] of [
    ['recent', '最近使用', 'recent://'],
    ['trash', isWindows() ? '回收站' : '废纸篓', 'trash://'],
  ]) {
    if (!result.some((item) => item.kind === kind))
      result.push({
        kind,
        name,
        path,
        available: 0,
        total: 0,
        removable: false,
        filesystem: '',
        device: '',
        mounted: true,
        usbfs: false,
      })
  }
  return result.map((place) => ({ ...place, name: t(place.name) }))
}

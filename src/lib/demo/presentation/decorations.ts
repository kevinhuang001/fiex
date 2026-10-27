// Mirrors Core's presentation DTO for the isolated browser fixture.
import { t } from '@/i18n'
import { tagHex } from '@/lib/tags'
import type { DecorationBadge, Entry, EntryDecoration } from '@/lib/types'

export function demoDecorations(entries: Entry[], plugins: EntryDecoration[]): EntryDecoration[] {
  const byPath = new Map(plugins.map((value) => [value.path, value]))
  for (const entry of entries) {
    const badges: DecorationBadge[] = entry.tags
      .slice(0, 5)
      .map((tag) => ({ text: tag.name, tooltip: tag.name, color: tagHex(tag), dot: true }))
    if (entry.readonly) badges.push({ text: t('只读'), tooltip: t('只读'), color: 'gray' })
    if (entry.sync) {
      const states = {
        synced: { text: t('已缓存'), icon: 'cloud-check', color: 'green' },
        cloud: { text: t('未缓存'), icon: 'cloud-download', color: 'gray' },
        pinned: { text: t('始终保留在本地'), icon: 'pin', color: 'purple' },
        pending: { text: t('等待上传'), icon: 'cloud-check', color: 'orange' },
        conflict: { text: t('同步冲突'), icon: 'cloud-check', color: 'red' },
      } as const
      const value = states[entry.sync]
      badges.push({ ...value, tooltip: value.text, status: entry.sync })
      if (entry.uploaded && (entry.hasCache || entry.pinned))
        badges.push({
          text: t('已上传'),
          tooltip: t('已上传'),
          color: 'blue',
          status: 'uploaded',
          icon: 'cloud-upload',
        })
    }
    if (badges.length) {
      const plugin = byPath.get(entry.path)
      byPath.set(entry.path, {
        path: entry.path,
        badges: [],
        ...plugin,
        nativeBadges: badges,
        providers: ['Fiex', ...(plugin?.providers || [])],
      })
    }
  }
  return [...byPath.values()]
}

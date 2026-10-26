import { t } from '@/i18n'
import type { Tag } from './types'

export interface TagPreset extends Tag {
  hex: string
}

export const TAG_PRESETS: TagPreset[] = [
  {
    get name() {
      return t('灰色')
    },
    color: 1,
    hex: '#8e8e93',
  },
  {
    get name() {
      return t('绿色')
    },
    color: 2,
    hex: '#34c759',
  },
  {
    get name() {
      return t('紫色')
    },
    color: 3,
    hex: '#af52de',
  },
  {
    get name() {
      return t('蓝色')
    },
    color: 4,
    hex: '#007aff',
  },
  {
    get name() {
      return t('黄色')
    },
    color: 5,
    hex: '#ffcc00',
  },
  {
    get name() {
      return t('红色')
    },
    color: 6,
    hex: '#ff3b30',
  },
  {
    get name() {
      return t('橙色')
    },
    color: 7,
    hex: '#ff9500',
  },
]

const COLORS = [
  '#8e8e93',
  '#8e8e93',
  '#34c759',
  '#af52de',
  '#007aff',
  '#ffcc00',
  '#ff3b30',
  '#ff9500',
]

function tagColor(color: number): string {
  return COLORS[color] || COLORS[0]
}

/* A wider palette for tags created in Fiex. Finder only persists the seven
   built-in colours, so `nearestFinderColor` picks the closest one for disk,
   while `hex` keeps the exact colour for the sidebar. */
export const TAG_PALETTE = [
  '#8e8e93',
  '#34c759',
  '#af52de',
  '#007aff',
  '#ffcc00',
  '#ff3b30',
  '#ff9500',
  '#00c7be',
  '#30d158',
  '#64d2ff',
  '#5e5ce6',
  '#bf5af2',
  '#ff375f',
  '#ff9f0a',
  '#32ade6',
  '#66d4cf',
  '#ffd60a',
  '#a2845e',
  '#6c7a89',
  '#e06c75',
]

function rgb(hex: string): [number, number, number] {
  const value = hex.replace('#', '')
  return [
    parseInt(value.slice(0, 2), 16),
    parseInt(value.slice(2, 4), 16),
    parseInt(value.slice(4, 6), 16),
  ]
}

export function nearestFinderColor(hex: string): number {
  const [r, g, b] = rgb(hex)
  let best = 4
  let bestDistance = Number.POSITIVE_INFINITY
  for (let index = 1; index <= 7; index += 1) {
    const [cr, cg, cb] = rgb(tagColor(index))
    const distance = (r - cr) ** 2 + (g - cg) ** 2 + (b - cb) ** 2
    if (distance < bestDistance) {
      bestDistance = distance
      best = index
    }
  }
  return best
}

export function tagHex(tag: { color: number; hex?: string }): string {
  return tag.hex || tagColor(tag.color)
}

/** Validate a persisted tag catalogue, dropping invalid or duplicate names. */
export function normalizeTags(value: unknown): Tag[] | null {
  if (!Array.isArray(value)) return null
  const seen = new Set<string>()
  const tags: Tag[] = []
  for (const item of value) {
    if (!item || typeof item !== 'object') continue
    const { name, color, hex } = item as Partial<Tag>
    if (typeof name !== 'string' || !name.trim()) continue
    if (typeof color !== 'number' || !Number.isInteger(color) || color < 1 || color > 7) continue
    const clean = name.trim()
    if (seen.has(clean)) continue
    seen.add(clean)
    const tag: Tag = { name: clean, color }
    if (typeof hex === 'string' && /^#[0-9a-f]{6}$/i.test(hex)) tag.hex = hex.toLowerCase()
    tags.push(tag)
  }
  return tags
}

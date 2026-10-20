import type { Tag } from './types'

export interface TagPreset extends Tag {
  hex: string
}

export const TAG_PRESETS: TagPreset[] = [
  { name: '灰色', color: 1, hex: '#8e8e93' },
  { name: '绿色', color: 2, hex: '#34c759' },
  { name: '紫色', color: 3, hex: '#af52de' },
  { name: '蓝色', color: 4, hex: '#007aff' },
  { name: '黄色', color: 5, hex: '#ffcc00' },
  { name: '红色', color: 6, hex: '#ff3b30' },
  { name: '橙色', color: 7, hex: '#ff9500' },
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

export function tagColor(color: number): string {
  return COLORS[color] || COLORS[0]
}

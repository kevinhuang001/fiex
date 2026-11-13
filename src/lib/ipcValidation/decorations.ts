import type {
  DecorationBadge,
  DecorationField,
  DecorationStyle,
  DecorationTarget,
  EntryDecoration,
} from '../types'
import { objectResult, parseJsonValue } from './json'

export function parseEntryDecorations(value: unknown): EntryDecoration[] {
  const json = parseJsonValue(value)
  if (!Array.isArray(json)) throw new TypeError('Invalid decorations list')
  return json.map(parseEntryDecoration)
}

const decorationTargets = ['name', 'modified', 'size', 'kind', 'row'] as const
const decorationTargetSet = new Set<string>(decorationTargets)
const decorationStatusValues: NonNullable<DecorationBadge['status']>[] = [
  'synced',
  'partial',
  'cloud',
  'pinned',
  'pending',
  'conflict',
  'uploaded',
] as const
const decorationStatusSet = new Set<string>(decorationStatusValues)
const decorationIconValues: NonNullable<DecorationBadge['icon']>[] = [
  'cloud-check',
  'cloud-download',
  'cloud-upload',
  'pin',
]
const decorationIconSet = new Set<string>(decorationIconValues)

function parseEntryDecoration(value: unknown): EntryDecoration {
  const decoration = objectResult(value)
  if (
    !hasOnlyKeys(decoration, [
      'path',
      'label',
      'description',
      'fields',
      'icon',
      'iconPath',
      'providers',
      'nativeBadges',
    ]) ||
    typeof decoration.path !== 'string'
  )
    throw new TypeError('Invalid decoration')

  if (
    decoration.providers !== undefined &&
    (!Array.isArray(decoration.providers) ||
      !decoration.providers.every((provider) => typeof provider === 'string'))
  )
    throw new TypeError('Invalid decoration providers')

  const label = optionalDecorationString(decoration.label, 'label')
  const description = optionalDecorationString(decoration.description, 'description')
  const icon = optionalDecorationString(decoration.icon, 'icon')
  const iconPath = optionalDecorationString(decoration.iconPath, 'iconPath')
  return {
    path: decoration.path,
    ...(label === undefined ? {} : { label }),
    ...(description === undefined ? {} : { description }),
    ...(decoration.fields === undefined
      ? {}
      : { fields: parseDecorationFields(decoration.fields) }),
    ...(icon === undefined ? {} : { icon }),
    ...(iconPath === undefined ? {} : { iconPath }),
    ...(Array.isArray(decoration.providers) ? { providers: decoration.providers } : {}),
    ...(decoration.nativeBadges === undefined
      ? {}
      : { nativeBadges: parseDecorationBadges(decoration.nativeBadges, true) }),
  }
}

function parseDecorationFields(value: unknown): Partial<Record<DecorationTarget, DecorationField>> {
  const fields = objectResult(value)
  const parsed: Partial<Record<DecorationTarget, DecorationField>> = {}
  for (const [target, field] of Object.entries(fields)) {
    if (!isDecorationTarget(target)) throw new TypeError('Invalid decoration target')
    parsed[target] = parseDecorationField(field)
  }
  return parsed
}

function parseDecorationField(value: unknown): DecorationField {
  const field = objectResult(value)
  if (!hasOnlyKeys(field, ['badges', 'style'])) throw new TypeError('Invalid decoration field')
  if (field.badges !== undefined && !Array.isArray(field.badges))
    throw new TypeError('Invalid decoration field badges')
  return {
    ...(Array.isArray(field.badges) ? { badges: parseDecorationBadges(field.badges, false) } : {}),
    ...(field.style === undefined ? {} : { style: parseDecorationStyle(field.style) }),
  }
}

function parseDecorationBadges(value: unknown, native: boolean): DecorationBadge[] {
  if (!Array.isArray(value)) throw new TypeError('Invalid decoration badges')
  return value.map((badge) => parseDecorationBadge(badge, native))
}

function parseDecorationBadge(value: unknown, native: boolean): DecorationBadge {
  const badge = objectResult(value)
  const allowed = native
    ? ['text', 'color', 'tooltip', 'dot', 'status', 'icon']
    : ['text', 'color', 'tooltip']
  const tooltip = optionalDecorationString(badge.tooltip, 'badge tooltip')
  const status = badge.status
  const icon = badge.icon
  if (
    !hasOnlyKeys(badge, allowed) ||
    typeof badge.text !== 'string' ||
    typeof badge.color !== 'string' ||
    (badge.tooltip !== undefined && badge.tooltip !== null && typeof badge.tooltip !== 'string') ||
    (native && badge.dot !== undefined && typeof badge.dot !== 'boolean') ||
    (native && status !== undefined && status !== null && !isDecorationStatus(status)) ||
    (native && icon !== undefined && icon !== null && !isDecorationIcon(icon))
  )
    throw new TypeError('Invalid decoration badge')

  return {
    text: badge.text,
    color: badge.color,
    ...(tooltip === undefined ? {} : { tooltip }),
    ...(native && typeof badge.dot === 'boolean' ? { dot: badge.dot } : {}),
    ...(native && isDecorationStatus(status) ? { status } : {}),
    ...(native && isDecorationIcon(icon) ? { icon } : {}),
  }
}

function parseDecorationStyle(value: unknown): DecorationStyle {
  const style = objectResult(value)
  const keys = ['color', 'fontWeight', 'fontStyle', 'backgroundLight', 'backgroundDark'] as const
  if (!hasOnlyKeys(style, keys)) throw new TypeError('Invalid decoration style')
  const color = optionalDecorationString(style.color, 'style color')
  const fontWeight = optionalDecorationString(style.fontWeight, 'style fontWeight')
  const fontStyle = optionalDecorationString(style.fontStyle, 'style fontStyle')
  const backgroundLight = optionalDecorationString(style.backgroundLight, 'style backgroundLight')
  const backgroundDark = optionalDecorationString(style.backgroundDark, 'style backgroundDark')
  return {
    ...(color === undefined ? {} : { color }),
    ...(fontWeight === undefined ? {} : { fontWeight }),
    ...(fontStyle === undefined ? {} : { fontStyle }),
    ...(backgroundLight === undefined ? {} : { backgroundLight }),
    ...(backgroundDark === undefined ? {} : { backgroundDark }),
  }
}

function optionalDecorationString(value: unknown, field: string): string | undefined {
  if (value === undefined || value === null) return undefined
  if (typeof value !== 'string') throw new TypeError(`Invalid decoration ${field}`)
  return value
}

function hasOnlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key))
}

function isDecorationTarget(value: string): value is DecorationTarget {
  return decorationTargetSet.has(value)
}

function isDecorationStatus(value: unknown): value is NonNullable<DecorationBadge['status']> {
  return typeof value === 'string' && decorationStatusSet.has(value)
}

function isDecorationIcon(value: unknown): value is NonNullable<DecorationBadge['icon']> {
  return typeof value === 'string' && decorationIconSet.has(value)
}

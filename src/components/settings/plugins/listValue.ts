import type { PluginField } from '@/lib/plugins'

export function asList(value: unknown, field: PluginField): string[] {
  const fallback = field.defaultValue
  if (Array.isArray(value)) return value.map((item) => String(item))
  if (typeof value === 'string' && value.trim())
    return value
      .split(/[\n,，]/)
      .map((item) => item.trim())
      .filter(Boolean)
  if (Array.isArray(fallback)) return fallback.map((item) => String(item))
  if (typeof fallback === 'string' && fallback.trim())
    return fallback
      .split(/[\n,，]/)
      .map((item) => item.trim())
      .filter(Boolean)
  return []
}

import type { DecorationField, DecorationTarget, Entry, EntryDecoration } from '@/lib/types'
import type { DecorationProvider } from './types'
import { connectionDecorationProvider } from './providers/connection'
import { tagDecorationProvider } from './providers/tags'
import { readonlyDecorationProvider } from './providers/readonly'

export const builtinDecorationProviders: readonly DecorationProvider[] = [
  tagDecorationProvider,
  readonlyDecorationProvider,
  connectionDecorationProvider,
]
const targets: DecorationTarget[] = ['name', 'modified', 'size', 'kind', 'row']

/** Native plugin results have already passed Core's validation. Builtins and
 * plugins share the same presentation DTO; plugins override styles/labels,
 * while builtin status badges never consume the plugin badge allowance. */
export function resolveDecorations(
  entries: Entry[],
  plugins: Record<string, EntryDecoration>,
  providers: readonly DecorationProvider[] = builtinDecorationProviders,
): Record<string, EntryDecoration> {
  const result: Record<string, EntryDecoration> = {}
  const ordered = [...providers].sort((a, b) => a.priority - b.priority || a.id.localeCompare(b.id))
  for (const entry of entries) {
    const values = ordered
      .map((provider) => provider.resolve(entry))
      .filter((value): value is EntryDecoration => Boolean(value))
    const plugin = plugins[entry.path]
    if (plugin) values.push(plugin)
    if (!values.length) continue
    const fields: EntryDecoration['fields'] = {}
    let combined: EntryDecoration = { path: entry.path, badges: [], fields, providers: [] }
    for (const value of values) {
      combined = {
        ...combined,
        ...Object.fromEntries(
          Object.entries(value).filter(
            ([key, val]) =>
              !['fields', 'badges', 'style', 'providers'].includes(key) && val != null,
          ),
        ),
      }
      combined.providers = [...new Set([...(combined.providers || []), ...(value.providers || [])])]
      for (const target of targets) {
        // Match Core: legacy name badges/style are a fallback for older DTOs.
        const source =
          value.fields?.[target] ||
          (target === 'name' ? { badges: value.badges, style: value.style } : undefined)
        if (!source) continue
        const previous: DecorationField = fields[target] || {}
        fields[target] = {
          badges: [...(previous.badges || []), ...(source.badges || [])],
          style: { ...previous.style, ...source.style },
        }
      }
    }
    combined.fields = fields
    combined.badges = fields.name?.badges || []
    combined.style = fields.name?.style
    result[entry.path] = combined
  }
  return result
}

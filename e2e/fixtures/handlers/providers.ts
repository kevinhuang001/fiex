import { notHandled } from './shared'
let preferences: Record<string, { order: string[]; disabled: string[] }> = {}
const plugin = (id: string, name: string, modes?: string[]) => ({
  id,
  name,
  defaultPriority: 100,
  builtin: false,
  available: true,
  modes,
})
const fallback = (id: string, name: string) => ({
  id,
  name,
  defaultPriority: -1000,
  builtin: true,
  locked: true,
  available: true,
})
export function handleProviders(command: string, args: Record<string, unknown>): unknown {
  switch (command) {
    case 'provider_catalog':
      return {
        features: {
          search: [
            plugin('org.fiex.everything', 'Everything', ['name']),
            fallback('core:search', 'Fiex'),
          ],
          preview: [
            plugin('org.fiex.office-preview', 'Office Preview'),
            plugin('org.fiex.data-preview', 'Data Preview'),
            fallback('core:preview', 'Fiex'),
          ],
          info: [plugin('org.fiex.checksum', 'Checksum'), fallback('core:info', 'Fiex')],
          decorations: [plugin('org.fiex.git', 'Git'), fallback('core:decorations', 'Fiex')],
          contextMenus: [plugin('org.fiex.archive', 'Archive'), fallback('core:menus', 'Fiex')],
          hooks: [plugin('org.fiex.archive', 'Archive')],
        },
      }
    case 'provider_preferences':
      return structuredClone(preferences)
    case 'set_provider_preference':
      preferences[String(args.feature)] = {
        order: args.order as string[],
        disabled: args.disabled as string[],
      }
      return structuredClone(preferences)
    case 'reset_provider_preference':
      delete preferences[String(args.feature)]
      return structuredClone(preferences)
    default:
      return notHandled
  }
}

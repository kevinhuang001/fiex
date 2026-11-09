import { fixturePlugins } from '../../plugins'
import { notHandled } from '../shared'
import { fixturePreviewNames } from './state'

async function resolveFixturePreview(a: Record<string, unknown>) {
  const path = String(a.path || '')
  const name = path.split(/[\\/]/).pop() || ''
  const extension = name.includes('.') ? name.split('.').pop()?.toLowerCase() || '' : ''
  const providerId = String(a.providerId || '')
  const candidates = (await handlePluginPreview('preview_candidates', {
    extension,
    mime: '',
    isDir: false,
  })) as Array<{ id: string; provider: string; priority: number }>
  const provider = candidates.find((value) => value.id === providerId) || candidates[0]
  if (!provider) throw new Error('没有可用的预览方式')
  if (provider.id.startsWith('plugin:')) {
    const pluginId = provider.id.slice('plugin:'.length)
    const contributionId = pluginId
    const slug = pluginId.split('.').pop() || ''
    const entry = pluginId === 'org.fiex.archive' ? 'preview.html' : 'index.html'
    const response = await fetch(`/__fiex_official_plugin/${slug}/${entry}`)
    if (!response.ok) throw new Error(await response.text())
    const html = await response.text()
    const sessionId = crypto.randomUUID()
    fixturePreviewNames.set(sessionId, name)
    // `?slowPreview` keeps the loading page visible long enough to inspect it.
    if (new URLSearchParams(location.search).has('slowPreview')) {
      await new Promise((resolve) => setTimeout(resolve, 900))
    }
    return {
      providerId: provider.id,
      provider: provider.provider,
      priority: provider.priority,
      view: {
        kind: 'html',
        html,
        sessionId,
        pluginId,
        contributionId,
        context: {
          sessionId,
          targets: [
            {
              ref: { mountId: 'local', nodeId: `fiex-resource:${sessionId}` },
              metadata: { name, kind: 'file', extension },
            },
          ],
        },
      },
    }
  }
  if (provider.id === 'builtin:text') {
    const text = name.endsWith('.json')
      ? '{\n  "name": "Fiex",\n  "preview": "demo",\n  "theme": "light"\n}'
      : '# Fiex\n\n留一点空间，给重要的文件。\n\n支持四种视图、快速查找与空格预览。'
    return {
      providerId: provider.id,
      provider: provider.provider,
      priority: provider.priority,
      view: { kind: 'text', text, truncated: false },
    }
  }
  if (provider.id === 'builtin:pdf') {
    return {
      providerId: provider.id,
      provider: provider.provider,
      priority: provider.priority,
      view: { kind: 'pdf', path: '/demo/sample.pdf', name },
    }
  }
  return {
    providerId: provider.id,
    provider: provider.provider,
    priority: provider.priority,
    view: {
      kind: 'fallback',
      name,
      extension,
      message: 'demo 未内置该类型的媒体样本',
    },
  }
}

export async function handlePluginPreview(
  command: string,
  a: Record<string, unknown>,
): Promise<unknown> {
  switch (command) {
    case 'preview_candidates': {
      const extension = String(a.extension || '').toLowerCase()
      const mime = String(a.mime || '').toLowerCase()
      const isDir = Boolean(a.isDir)
      const textExt = new Set([
        'txt',
        'md',
        'markdown',
        'json',
        'csv',
        'tsv',
        'css',
        'html',
        'htm',
        'xml',
        'yml',
        'yaml',
        'rs',
        'ts',
        'tsx',
        'js',
        'jsx',
        'py',
        'go',
        'java',
        'c',
        'cpp',
        'h',
        'sh',
        'log',
        'toml',
        'ini',
      ])
      const values: Array<{ id: string; provider: string; priority: number; loading?: boolean }> =
        []
      if (!isDir && (mime.startsWith('text/') || textExt.has(extension)))
        values.push({ id: 'builtin:text', provider: 'Fiex', priority: 10 })
      if (!isDir && mime.startsWith('image/'))
        values.push({ id: 'builtin:image', provider: 'Fiex', priority: 20 })
      if (!isDir && (mime.startsWith('video/') || mime.startsWith('audio/')))
        values.push({ id: 'builtin:media', provider: 'Fiex', priority: 30 })
      if (!isDir && extension === 'pdf')
        values.push({ id: 'builtin:pdf', provider: 'Fiex', priority: 40 })
      if (values.length === 0)
        values.push({ id: 'builtin:fallback', provider: 'Fiex', priority: 0 })
      for (const plugin of fixturePlugins) {
        if (!plugin.enabled) continue
        const contribution = plugin.extensions?.preview as
          { extensions?: string[]; priority?: number; loading?: unknown } | undefined
        if (!contribution) continue
        const extensions = Array.isArray(contribution.extensions)
          ? contribution.extensions.map((value: string) => value.toLowerCase())
          : []
        if (!extensions.includes(extension)) continue
        values.push({
          id: `plugin:${plugin.id}`,
          provider: plugin.name,
          priority: contribution.priority ?? 50,
          loading: Boolean(contribution.loading),
        })
      }
      return values.sort((left, right) => right.priority - left.priority)
    }
    case 'preview_loading': {
      const pluginId = String(a.pluginId || '')
      if (!pluginId.startsWith('org.fiex.')) return null
      const slug = pluginId.split('.').pop() || ''
      const response = await fetch(`/__fiex_official_plugin/${slug}/loading.html`)
      return response.ok ? await response.text() : null
    }
    case 'preview_open': {
      const values = (await handlePluginPreview('preview_candidates', {
        extension:
          String(a.path || '')
            .split('.')
            .pop() || '',
        mime: '',
        isDir: false,
      })) as Array<{ id: string; provider: string; priority: number }>
      const allowed = Array.isArray(a.providerOrder)
        ? a.providerOrder.filter((id): id is string => typeof id === 'string')
        : values.map((value) => value.id)
      const order = [String(a.providerId || ''), ...allowed].filter(
        (id, index, list) => id && list.indexOf(id) === index && allowed.includes(id),
      )
      for (const id of order) {
        try {
          return await resolveFixturePreview({ ...a, providerId: id })
        } catch {
          // Try the next registered preview provider.
        }
      }
      return {
        providerId: 'builtin:fallback',
        provider: 'Fiex',
        priority: 0,
        view: {
          kind: 'fallback',
          name:
            String(a.path || '')
              .split('/')
              .pop() || '',
        },
      }
    }
    default:
      return notHandled
  }
}

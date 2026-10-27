import { call } from '@/lib/api'
import type { ResourceContext, ResourceRef } from '../../../../sdk/ui/src/index'

const key = (reference: ResourceRef) => JSON.stringify([reference.mountId, reference.nodeId])
/** Candidate identities belong to one Core-owned panel. Merely describing or
 * choosing a resource grants nothing; only references named on submit enter
 * that invocation's captured selection. Host paths never cross into iframe UI. */
export class PanelResources {
  private values = new Map<string, { resource: ResourceContext; path: string }>()
  private byPath = new Map<string, ResourceContext>()
  constructor(private readonly pluginId: string) {}
  async describe(paths: string[]): Promise<ResourceContext[]> {
    const missing = [...new Set(paths)].filter((path) => !this.byPath.has(path))
    if (missing.length) {
      const resources = await call<ResourceContext[]>('plugin_describe_resources', {
        pluginId: this.pluginId,
        paths: missing,
      })
      resources.forEach((resource, index) => {
        this.byPath.set(missing[index], resource)
        this.values.set(key(resource.ref), { resource, path: missing[index] })
      })
    }
    return paths.map((path) => this.byPath.get(path)!)
  }
  path(reference: ResourceRef | undefined): string | undefined {
    return reference && this.values.get(key(reference))?.path
  }
  bindings(params: unknown): { ref: ResourceRef; path: string }[] {
    const found = new Map<string, { ref: ResourceRef; path: string }>()
    const visit = (value: unknown, depth: number) => {
      if (depth > 12 || !value || typeof value !== 'object') return
      if (Array.isArray(value)) {
        value.forEach((value) => visit(value, depth + 1))
        return
      }
      const record = value as Record<string, unknown>
      if (typeof record.mountId === 'string' && typeof record.nodeId === 'string') {
        const reference = record as ResourceRef
        const candidate = this.values.get(key(reference))
        if (candidate)
          found.set(key(reference), { ref: candidate.resource.ref, path: candidate.path })
      }
      Object.values(record).forEach((value) => visit(value, depth + 1))
    }
    visit(params, 0)
    return [...found.values()]
  }
}

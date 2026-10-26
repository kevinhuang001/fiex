import { demoEntries } from '../data'

// Marker returned by a domain handler when a command does not belong to it.
export const notHandled = Symbol('notHandled')

// Shared mutable runtime state: file operations replace or mutate the listing,
// and git init reads it. Keeping it here preserves the original module state.
export const state = { entries: demoEntries }

export const demoViews = new Map<string, { root: string; commit: string }>()
export function demoView(root: string, commit: string): string {
  const found = [...demoViews].find(([, value]) => value.root === root && value.commit === commit)
  if (found) return found[0]
  const uri = `fiex://mount/demo-${demoViews.size + 1}/`
  demoViews.set(uri, { root, commit })
  return uri
}

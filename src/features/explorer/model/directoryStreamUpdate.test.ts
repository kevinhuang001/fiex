import { describe, expect, it } from 'vitest'
import { decideDirectoryStreamUpdate } from './directoryStreamUpdate'
import type { Entry, Listing } from '@/lib/types'
import type { DirectoryStreamUpdate } from './directoryStreamUpdate'

function entry(path: string): Entry {
  return {
    path,
    name: path.split('/').at(-1) ?? path,
    isDir: false,
    isSymlink: false,
    size: 1,
    modified: 1,
    created: 1,
    extension: 'txt',
    mime: 'text/plain',
    linkTarget: null,
    tags: [],
    sync: null,
  }
}

function listing(path: string, entries: Entry[] = []): Listing {
  return { path, entries, warnings: [], allowedOps: [] }
}

function update(values: Partial<DirectoryStreamUpdate> = {}): DirectoryStreamUpdate {
  return {
    decorations: [],
    complete: true,
    presentationPending: false,
    revision: 1,
    ...values,
  }
}

function decide(
  path: string,
  value: DirectoryStreamUpdate,
  options: {
    previous?: {
      listing: Listing
      decorations: { path: string; label: string }[]
      revision: number
    } | null
    historyLength?: number
    pendingSelection?: { directory: string; target: string; generation: number } | null
  } = {},
) {
  return decideDirectoryStreamUpdate({
    path,
    update: value,
    previous: options.previous ?? null,
    sort: 'name',
    descending: false,
    historyLength: options.historyLength ?? 1,
    pendingSelection: options.pendingSelection ?? null,
  })
}

describe('decideDirectoryStreamUpdate', () => {
  it('ignores a stream revision older than the painted snapshot', () => {
    const result = decide('/docs', update({ revision: 3, listing: listing('/elsewhere') }), {
      previous: { listing: listing('/docs'), decorations: [], revision: 4 },
    })

    expect(result).toEqual({ kind: 'ignore' })
  })

  it('merges decoration-only patches and advances the snapshot revision', () => {
    const previous = {
      listing: listing('/docs', [entry('/docs/a.txt')]),
      decorations: [{ path: '/docs/a.txt', label: 'old' }],
      revision: 2,
    }
    const patch = { path: '/docs/b.txt', label: 'new' }
    const result = decide(
      '/docs',
      update({
        listing: undefined,
        complete: false,
        presentationPending: true,
        revision: 3,
        decorations: [patch],
      }),
      { previous },
    )

    expect(result.kind).toBe('patch')
    if (result.kind !== 'patch') return
    expect(result.snapshot.revision).toBe(3)
    expect(result.snapshot.decorations).toEqual([previous.decorations[0], patch])
    expect(result.patchDecorations).toEqual([patch])
    expect(result.presentationPending).toBe(true)
    expect(result.shouldApplyDecorations).toBe(true)
  })

  it('corrects the current path when a listing belongs to another location', () => {
    expect(decide('/requested', update({ listing: listing('/canonical') }))).toEqual({
      kind: 'navigate',
      path: '/canonical',
    })
  })

  it('selects a requested entry only after it appears in its directory listing', () => {
    const target = '/docs/target.txt'
    const result = decide('/docs', update({ listing: listing('/docs', [entry(target)]) }), {
      pendingSelection: { directory: '/docs', target, generation: 7 },
    })

    expect(result.kind).toBe('listing')
    if (result.kind !== 'listing') return
    expect(result.selection).toEqual({ kind: 'select', target, generation: 7 })
  })

  it('clears a pending selection as missing only after the listing is complete', () => {
    const pendingSelection = { directory: '/docs', target: '/docs/missing.txt', generation: 9 }
    const partial = decide('/docs', update({ listing: listing('/docs'), complete: false }), {
      pendingSelection,
    })
    const complete = decide('/docs', update({ listing: listing('/docs') }), { pendingSelection })

    expect(partial.kind === 'listing' && partial.selection).toEqual({ kind: 'none' })
    expect(complete.kind === 'listing' && complete.selection).toEqual({
      kind: 'missing',
      generation: 9,
    })
  })

  it('redirects from an empty Recent directory only on its completed first-history visit', () => {
    const updateValue = update({ listing: listing('recent://'), complete: true })
    const firstVisit = decide('recent://', updateValue, { historyLength: 1 })
    const laterVisit = decide('recent://', updateValue, { historyLength: 2 })
    const partial = decide('recent://', update({ listing: listing('recent://'), complete: false }))

    expect(firstVisit.kind === 'listing' && firstVisit.redirectHome).toBe(true)
    expect(laterVisit.kind === 'listing' && laterVisit.redirectHome).toBe(false)
    expect(partial.kind === 'listing' && partial.redirectHome).toBe(false)
  })
})

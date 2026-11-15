import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import type { Explorer } from '@/features/explorer/hooks/useExplorer'
import { PathToolbar } from './PathToolbar'

describe('PathToolbar drop target', () => {
  it('keeps the up button and breadcrumb separators outside the drop highlight', () => {
    const x = {
      path: '/Users/kevin/Documents',
      up: vi.fn(),
      globalSearch: false,
      setGlobalSearch: vi.fn(),
      searchMode: 'name',
      setSearchMode: vi.fn(),
      searching: false,
      query: '',
      setQuery: vi.fn(),
    } as unknown as Explorer
    const markup = renderToStaticMarkup(
      <PathToolbar
        x={x}
        crumbs={[
          { name: 'Users', path: '/Users' },
          { name: 'kevin', path: '/Users/kevin' },
          { name: 'Documents', path: '/Users/kevin/Documents' },
        ]}
        editing={false}
        draft=""
        setDraft={vi.fn()}
        startEdit={vi.fn()}
        submit={vi.fn()}
        stopEdit={vi.fn()}
        pathInput={{ current: null }}
        searchInput={{ current: null }}
        searchHint="⌘F"
        ariaLabel="Path"
        pathLabel="Folder path"
        searchLabel="Search"
      />,
    )

    expect(markup).toMatch(/<div class="path-toolbar"><button[^>]*>/)
    expect(markup).toMatch(/<nav class="breadcrumbs" aria-label="Path">/)
    expect(markup).toContain('<span><svg')
    expect(markup).toMatch(/<button[^>]*data-drop-folder="\/Users\/kevin"/)
    expect(markup).not.toContain('<span data-drop-folder=')
  })
})

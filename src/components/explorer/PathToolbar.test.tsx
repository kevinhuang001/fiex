import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import type { Explorer } from '@/hooks/useExplorer'
import { PathToolbar } from './PathToolbar'

describe('PathToolbar drop target', () => {
  it('keeps the up button outside the breadcrumb drop highlight', () => {
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
    expect(markup).toMatch(/<nav class="breadcrumbs" data-drop-folder="\/Users\/kevin\/Documents"/)
  })
})

import { t } from '@/i18n'
import { Button } from '@/components/ui/button'
import type { Explorer } from '@/hooks/useExplorer'
import { ArrowUp, ChevronRight, FileText, LoaderCircle, Search, Type, X } from 'lucide-react'
import type { FormEvent, RefObject } from 'react'

export type Crumb = { name: string; path: string }

export function PathToolbar({
  x,
  crumbs,
  editing,
  draft,
  setDraft,
  startEdit,
  submit,
  stopEdit,
  pathInput,
  searchInput,
  searchHint,
  ariaLabel,
  pathLabel,
  searchLabel,
  close,
  showGlobalSearch = true,
}: {
  x: Explorer
  crumbs: Crumb[]
  editing: boolean
  draft: string
  setDraft: (value: string) => void
  startEdit: () => void
  submit: (event: FormEvent) => void
  stopEdit: () => void
  pathInput: RefObject<HTMLInputElement | null>
  searchInput: RefObject<HTMLInputElement | null>
  searchHint: string
  ariaLabel: string
  pathLabel: string
  searchLabel: string
  close?: { label: string; title: string; className: string; onClose: () => void }
  showGlobalSearch?: boolean
}) {
  const dropFolder = /^[a-z][a-z0-9+.-]*:\/\//i.test(x.path) ? undefined : x.path
  return (
    <div className="path-toolbar" data-drop-folder={dropFolder}>
      <Button
        variant="ghost"
        size="icon"
        title={t('上一级')}
        aria-label={t('上一级')}
        onClick={x.up}
      >
        <ArrowUp />
      </Button>
      <nav
        className="breadcrumbs"
        aria-label={ariaLabel}
        onClick={(event) => {
          if (event.target === event.currentTarget) startEdit()
        }}
      >
        {editing ? (
          <form className="path-editor" onSubmit={submit}>
            <input
              ref={pathInput}
              className="path-input"
              aria-label={pathLabel}
              value={draft}
              spellCheck={false}
              onChange={(event) => setDraft(event.target.value)}
              onBlur={stopEdit}
              onKeyDown={(event) => {
                event.stopPropagation()
                if (event.key === 'Escape') {
                  event.preventDefault()
                  stopEdit()
                }
              }}
            />
          </form>
        ) : (
          <>
            {(() => {
              const visible =
                crumbs.length > 3
                  ? [
                      { name: '…', path: crumbs[crumbs.length - 4].path, ellipsis: true },
                      ...crumbs.slice(-3).map((crumb) => ({ ...crumb, ellipsis: false })),
                    ]
                  : crumbs.map((crumb) => ({ ...crumb, ellipsis: false }))
              return visible.map((crumb, index) => (
                <span key={crumb.path} data-drop-folder={crumb.path}>
                  {index > 0 && <ChevronRight size={11} />}
                  <button
                    title={crumb.ellipsis ? t('更上层') : crumb.path}
                    aria-label={crumb.ellipsis ? t('更上层') : undefined}
                    onClick={() =>
                      crumb.ellipsis || index === visible.length - 1
                        ? crumb.ellipsis
                          ? x.navigate(crumb.path)
                          : startEdit()
                        : x.navigate(crumb.path)
                    }
                  >
                    {crumb.name}
                  </button>
                </span>
              ))
            })()}
          </>
        )}
      </nav>
      <div className="search-box">
        {showGlobalSearch && (
          <button
            className="search-scope"
            title={x.globalSearch ? t('切换为当前文件夹搜索') : t('搜索所有可访问的文件')}
            aria-label={t('全局搜索')}
            aria-pressed={x.globalSearch}
            onClick={() => x.setGlobalSearch(!x.globalSearch)}
          >
            <Search size={15} />
          </button>
        )}
        <button
          className="search-scope"
          title={x.searchMode === 'content' ? t('切换为文件名搜索') : t('切换为文件内容搜索')}
          aria-label={t('搜索模式')}
          aria-pressed={x.searchMode === 'content'}
          onClick={() => x.setSearchMode(x.searchMode === 'content' ? 'name' : 'content')}
        >
          {x.searchMode === 'content' ? <FileText size={15} /> : <Type size={15} />}
        </button>
        <input
          ref={searchInput}
          aria-label={searchLabel}
          placeholder={
            x.searchMode === 'content'
              ? t('搜索文件内容')
              : x.globalSearch
                ? t('搜索所有可访问位置')
                : t('搜索此文件夹')
          }
          value={x.query}
          onChange={(event) => x.setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== 'Escape') return
            event.stopPropagation()
            x.setQuery('')
            event.currentTarget.blur()
          }}
        />
        {x.searching ? (
          <LoaderCircle className="spin" size={14} />
        ) : x.query ? (
          <button className="icon-button" aria-label={t('清除搜索')} onClick={() => x.setQuery('')}>
            <X size={13} />
          </button>
        ) : (
          <kbd>{searchHint}</kbd>
        )}
      </div>
      {close && (
        <button
          className={`icon-button ${close.className}`}
          title={close.title}
          aria-label={close.label}
          onClick={close.onClose}
        >
          <X size={14} />
        </button>
      )}
    </div>
  )
}

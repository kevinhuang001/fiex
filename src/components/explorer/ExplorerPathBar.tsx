import { Button } from '@/components/ui/button'
import type { AppController } from '@/hooks/useAppController'
import { ArrowUp, ChevronRight, HardDrive, LoaderCircle, Search, X } from 'lucide-react'
export function ExplorerPathBar({
  x,
  startPathEdit,
  editingPath,
  submitPath,
  pathInput,
  pathDraft,
  setPathDraft,
  setEditingPath,
  searchInput,
  crumbs,
  hint,
}: Pick<
  AppController,
  | 'x'
  | 'startPathEdit'
  | 'editingPath'
  | 'submitPath'
  | 'pathInput'
  | 'pathDraft'
  | 'setPathDraft'
  | 'setEditingPath'
  | 'searchInput'
  | 'crumbs'
  | 'hint'
>) {
  return (
    <>
      <div className="path-toolbar">
        <Button variant="ghost" size="icon" title="上一级" aria-label="上一级" onClick={x.up}>
          <ArrowUp />
        </Button>
        <nav
          className="breadcrumbs"
          aria-label="当前位置"
          onClick={(event) => {
            if (event.target === event.currentTarget) startPathEdit()
          }}
        >
          <button onClick={startPathEdit} title="编辑或复制路径" aria-label="编辑路径">
            <HardDrive size={13} />
          </button>
          {editingPath ? (
            <form className="path-editor" onSubmit={submitPath}>
              <input
                ref={pathInput}
                className="path-input"
                aria-label="文件夹路径"
                value={pathDraft}
                spellCheck={false}
                onChange={(event) => setPathDraft(event.target.value)}
                onBlur={() => setEditingPath(false)}
                onKeyDown={(event) => {
                  event.stopPropagation()
                  if (event.key === 'Escape') {
                    event.preventDefault()
                    setEditingPath(false)
                    searchInput.current?.focus()
                  }
                }}
              />
            </form>
          ) : (
            <>
              {crumbs.map((p, i) => (
                <span key={i}>
                  <ChevronRight size={11} />
                  <button
                    onClick={() => (i === crumbs.length - 1 ? startPathEdit() : x.navigate(p.path))}
                  >
                    {p.name}
                  </button>
                </span>
              ))}
            </>
          )}
        </nav>
        <div className="search-box">
          <Search size={15} />
          <input
            ref={searchInput}
            aria-label="搜索文件"
            placeholder="搜索此文件夹"
            value={x.query}
            onChange={(e) => x.setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key !== 'Escape') return
              e.stopPropagation()
              x.setQuery('')
              e.currentTarget.blur()
            }}
          />
          {x.searching ? (
            <LoaderCircle className="spin" size={14} />
          ) : x.query ? (
            <button className="icon-button" aria-label="清除搜索" onClick={() => x.setQuery('')}>
              <X size={13} />
            </button>
          ) : (
            <kbd>{hint('search')}</kbd>
          )}
        </div>
      </div>
    </>
  )
}

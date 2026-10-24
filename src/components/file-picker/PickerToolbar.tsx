import type { MouseEvent as ReactMouseEvent } from 'react'
import {
  ArrowLeft,
  ArrowRight,
  ChevronUp,
  LayoutGrid,
  List,
  MoreHorizontal,
  Search,
} from 'lucide-react'

export function PickerToolbar({
  directory,
  canBack,
  canForward,
  navigate,
  onBack,
  onForward,
  onUp,
  onMore,
  filter,
  onFilter,
  view,
  onToggleView,
}: {
  directory: string
  canBack: boolean
  canForward: boolean
  navigate(path: string): void
  onBack(): void
  onForward(): void
  onUp(): void
  onMore(event: ReactMouseEvent<HTMLButtonElement>): void
  filter: string
  onFilter(value: string): void
  view: 'list' | 'grid'
  onToggleView(): void
}) {
  const segments = directory.split('/').filter(Boolean)
  return (
    <div className="file-picker-toolbar">
      <button type="button" aria-label="后退" disabled={!canBack} onClick={onBack}>
        <ArrowLeft size={15} />
      </button>
      <button type="button" aria-label="前进" disabled={!canForward} onClick={onForward}>
        <ArrowRight size={15} />
      </button>
      <button type="button" aria-label="上一级" onClick={onUp}>
        <ChevronUp size={15} />
      </button>
      <nav className="file-picker-crumbs" aria-label="路径">
        {segments.length > 3 && (
          <button
            type="button"
            aria-label="更上层"
            title="更上层"
            onClick={() => navigate('/' + segments.slice(0, segments.length - 3).join('/'))}
          >
            …
          </button>
        )}
        {segments.slice(Math.max(0, segments.length - 3)).map((segment, position) => {
          const index = segments.length - Math.min(3, segments.length) + position
          return (
            <button
              key={index}
              type="button"
              onClick={() => navigate('/' + segments.slice(0, index + 1).join('/'))}
            >
              {segment}
            </button>
          )
        })}
      </nav>
      <button type="button" aria-label="更多" title="新建、排序、分组" onClick={onMore}>
        <MoreHorizontal size={15} />
      </button>
      <label className="file-picker-search">
        <Search size={13} />
        <input
          value={filter}
          onChange={(event) => onFilter(event.target.value)}
          placeholder="搜索"
          aria-label="搜索"
        />
      </label>
      <button
        type="button"
        aria-label={view === 'list' ? '图标视图' : '列表视图'}
        onClick={onToggleView}
      >
        {view === 'list' ? <LayoutGrid size={15} /> : <List size={15} />}
      </button>
    </div>
  )
}

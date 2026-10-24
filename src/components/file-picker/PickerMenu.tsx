import type { RefObject } from 'react'
import { FolderPlus, LayoutGrid, List, Pencil, RotateCw, Trash2 } from 'lucide-react'
import type { Entry } from '@/lib/types'
import type { GroupKey, SortKey } from './types'

export interface PickerMenuState {
  x: number
  y: number
  entry?: Entry
  kind: 'row' | 'blank'
}

export interface PickerMenuActions {
  open(entry: Entry): void
  rename(entry: Entry): void
  trash(entry: Entry): void
  newFolder(): void
  refresh(): void
  toggleView(): void
  sortBy(key: SortKey): void
  groupBy(key: GroupKey): void
  close(): void
}

const SORT_LABELS: Record<SortKey, string> = {
  name: '名称',
  modified: '修改日期',
  size: '大小',
  kind: '类型',
}
const GROUP_LABELS: Record<GroupKey, string> = {
  none: '不分组',
  kind: '类型',
  modified: '修改日期',
  size: '大小',
}

export function PickerMenu({
  menu,
  sort,
  group,
  view,
  position,
  menuRef,
  actions,
}: {
  menu: PickerMenuState
  sort: { key: SortKey; direction: 'asc' | 'desc' }
  group: GroupKey
  view: 'list' | 'grid'
  position: { left: number; top: number }
  menuRef: RefObject<HTMLDivElement | null>
  actions: PickerMenuActions
}) {
  return (
    <div
      ref={menuRef}
      className="file-picker-menu"
      style={position}
      role="menu"
      onPointerDown={(event) => event.stopPropagation()}
    >
      {menu.kind === 'row' && menu.entry && (
        <>
          <button type="button" role="menuitem" onClick={() => actions.open(menu.entry!)}>
            打开
          </button>
          <button type="button" role="menuitem" onClick={() => actions.rename(menu.entry!)}>
            <Pencil size={12} /> 重命名…
          </button>
          <button
            type="button"
            role="menuitem"
            className="danger"
            onClick={() => actions.trash(menu.entry!)}
          >
            <Trash2 size={12} /> 移到废纸篓
          </button>
        </>
      )}
      {menu.kind === 'blank' && (
        <>
          <button type="button" role="menuitem" onClick={() => actions.newFolder()}>
            <FolderPlus size={12} /> 新建文件夹
          </button>
          <button type="button" role="menuitem" onClick={() => actions.refresh()}>
            <RotateCw size={12} /> 刷新
          </button>
          <button type="button" role="menuitem" onClick={() => actions.toggleView()}>
            {view === 'list' ? <LayoutGrid size={12} /> : <List size={12} />}{' '}
            {view === 'list' ? '图标视图' : '列表视图'}
          </button>
          <div className="file-picker-menu-label">排序方式</div>
          {(Object.keys(SORT_LABELS) as SortKey[]).map((key) => (
            <button
              key={key}
              type="button"
              role="menuitemradio"
              aria-checked={sort.key === key}
              onClick={() => actions.sortBy(key)}
            >
              {SORT_LABELS[key]}
              {sort.key === key ? (sort.direction === 'asc' ? ' ↑' : ' ↓') : ''}
            </button>
          ))}
          <div className="file-picker-menu-label">分组方式</div>
          {(Object.keys(GROUP_LABELS) as GroupKey[]).map((key) => (
            <button
              key={key}
              type="button"
              role="menuitemradio"
              aria-checked={group === key}
              onClick={() => actions.groupBy(key)}
            >
              {GROUP_LABELS[key]}
              {group === key ? ' ✓' : ''}
            </button>
          ))}
        </>
      )}
    </div>
  )
}

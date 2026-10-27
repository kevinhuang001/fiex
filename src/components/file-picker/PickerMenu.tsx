import { t } from '@/i18n'
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
  get name() {
    return t('名称')
  },
  get modified() {
    return t('修改日期')
  },
  get size() {
    return t('大小')
  },
  get kind() {
    return t('类型')
  },
}
const GROUP_LABELS: Record<GroupKey, string> = {
  get none() {
    return t('不分组')
  },
  get kind() {
    return t('类型')
  },
  get modified() {
    return t('修改日期')
  },
  get size() {
    return t('大小')
  },
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
  const run = (action: () => void) => () => {
    actions.close()
    action()
  }
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
          <button type="button" role="menuitem" onClick={run(() => actions.open(menu.entry!))}>
            {t('打开')}
          </button>
          <button type="button" role="menuitem" onClick={run(() => actions.rename(menu.entry!))}>
            <Pencil size={12} /> {t('重命名…')}
          </button>
          <button
            type="button"
            role="menuitem"
            className="danger"
            onClick={run(() => actions.trash(menu.entry!))}
          >
            <Trash2 size={12} /> {t('移到废纸篓')}
          </button>
        </>
      )}
      {menu.kind === 'blank' && (
        <>
          <button type="button" role="menuitem" onClick={run(() => actions.newFolder())}>
            <FolderPlus size={12} /> {t('新建文件夹')}
          </button>
          <button type="button" role="menuitem" onClick={run(() => actions.refresh())}>
            <RotateCw size={12} /> {t('刷新')}
          </button>
          <button type="button" role="menuitem" onClick={run(() => actions.toggleView())}>
            {view === 'list' ? <LayoutGrid size={12} /> : <List size={12} />}{' '}
            {view === 'list' ? t('图标视图') : t('列表视图')}
          </button>
          <div className="file-picker-menu-label">{t('排序方式')}</div>
          {(Object.keys(SORT_LABELS) as SortKey[]).map((key) => (
            <button
              key={key}
              type="button"
              role="menuitemradio"
              aria-checked={sort.key === key}
              onClick={run(() => actions.sortBy(key))}
            >
              {SORT_LABELS[key]}
              {sort.key === key ? (sort.direction === 'asc' ? ' ↑' : ' ↓') : ''}
            </button>
          ))}
          <div className="file-picker-menu-label">{t('分组方式')}</div>
          {(Object.keys(GROUP_LABELS) as GroupKey[]).map((key) => (
            <button
              key={key}
              type="button"
              role="menuitemradio"
              aria-checked={group === key}
              onClick={run(() => actions.groupBy(key))}
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

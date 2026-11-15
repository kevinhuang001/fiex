import { t } from '@/i18n'
import type { Explorer } from '@/features/explorer/hooks/useExplorer'
import type { FormEvent, RefObject } from 'react'
import { PathToolbar, type Crumb } from './PathToolbar'
export interface ExplorerPathBarProps {
  x: Explorer
  startPathEdit: () => void
  editingPath: boolean
  submitPath: (event: FormEvent) => void
  pathInput: RefObject<HTMLInputElement | null>
  pathDraft: string
  setPathDraft: (value: string) => void
  setEditingPath: (editing: boolean) => void
  searchInput: RefObject<HTMLInputElement | null>
  crumbs: Crumb[]
  searchHint: string
  split: boolean
  closePane: (side?: 'left' | 'right') => void
}

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
  searchHint,
  split,
  closePane,
}: ExplorerPathBarProps) {
  return (
    <PathToolbar
      x={x}
      crumbs={crumbs}
      editing={editingPath}
      draft={pathDraft}
      setDraft={setPathDraft}
      startEdit={startPathEdit}
      submit={submitPath}
      stopEdit={() => setEditingPath(false)}
      pathInput={pathInput}
      searchInput={searchInput}
      searchHint={searchHint}
      ariaLabel={t('当前位置')}
      pathLabel={t('文件夹路径')}
      searchLabel={t('搜索文件')}
      close={
        split
          ? {
              label: t('关闭左侧面板'),
              title: t('关闭此面板'),
              className: 'pane-close-main',
              onClose: () => closePane('left'),
            }
          : undefined
      }
    />
  )
}

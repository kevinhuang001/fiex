import type { AppController } from '@/hooks/useAppController'
import { PathToolbar } from './PathToolbar'
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
  split,
  closePane,
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
  | 'split'
  | 'closePane'
>) {
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
      searchHint={hint('search')}
      ariaLabel="当前位置"
      searchLabel="搜索文件"
      close={
        split
          ? {
              label: '关闭左侧面板',
              title: '关闭此面板',
              className: 'pane-close-main',
              onClose: () => closePane('left'),
            }
          : undefined
      }
    />
  )
}

import { t } from '@/i18n'
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

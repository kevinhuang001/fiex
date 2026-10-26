import { t, getLocale } from '@/i18n'
import type { AppController } from '@/hooks/useAppController'
import type { Entry } from '@/lib/types'
import { size } from '@/lib/files'
import { ListChecks, TerminalSquare, X } from 'lucide-react'
export function ExplorerStatusBar({
  x,
  chosenAll,
  selectedAll,
  clipboard,
  setClipboard,
  workspace,
}: Pick<
  AppController,
  'x' | 'chosenAll' | 'selectedAll' | 'clipboard' | 'setClipboard' | 'workspace'
> & { chosenAll: Entry[]; selectedAll: string[] }) {
  return (
    <>
      <footer className="statusbar" data-workspace-drop="true">
        <div className="statusbar-row">
          <span>
            {x.entries.length.toLocaleString(getLocale())} {t('个项目')}
            {selectedAll.length > 0 && t(' · 已选择 {0} 项', selectedAll.length)}
            {chosenAll.length > 0 &&
              !chosenAll.some((e) => e.isDir) &&
              ` · ${size(chosenAll.reduce((a, e) => a + e.size, 0))}`}
          </span>
          {x.tagFilter && (
            <span>
              {t('标签：')}
              {x.tagFilter}
            </span>
          )}
          <span>{x.searchStats}</span>
          <div className="toolbar-spacer" />
          {clipboard && (
            <button title={t('清除剪贴板')} onClick={() => setClipboard(null)}>
              {clipboard.moving ? t('剪切') : t('复制')} {clipboard.paths.length} {t('项')}
              <X size={11} />
            </button>
          )}
          <button
            className="statusbar-terminal"
            title={t('打开终端')}
            aria-label={t('打开终端')}
            onClick={() =>
              window.dispatchEvent(
                new CustomEvent('fiex:show-terminal-panel', { detail: { cwd: x.path } }),
              )
            }
          >
            <TerminalSquare size={13} />
            {t('终端')}
          </button>
          <button
            className="workspace-toggle"
            aria-expanded={workspace.open}
            aria-label={t('工作区')}
            onClick={() => workspace.setOpen(!workspace.open)}
          >
            <ListChecks size={13} />
            {t('工作区')}
            <span className="workspace-count">{workspace.items.length}</span>
          </button>
        </div>
      </footer>
    </>
  )
}

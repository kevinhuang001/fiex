import { useEffect, useMemo, useState } from 'react'
import {
  CheckCircle2,
  ChevronDown,
  FileCode2,
  FolderGit2,
  GitBranch,
  GitCommitHorizontal,
  GitFork,
  LoaderCircle,
  Minus,
  Plus,
  RefreshCw,
  X,
} from 'lucide-react'
import { basename } from '@/lib/files'
import { gitState } from '@/lib/git'
import { useGitRepository } from '@/hooks/useGitRepository'
import type { GitStatus } from '@/lib/types'
import { Button } from './ui/button'
import { CommitPicker } from './git/CommitPicker'
import { GitRemote } from './git/GitRemote'
import { CommitDialog } from './git/CommitDialog'

interface Change {
  path: string
  code: string
  letter: string
  label: string
  staged: boolean
  unstaged: boolean
}
interface Props {
  open: boolean
  path: string
  git: GitStatus | null
  loading?: boolean
  allowInit?: boolean
  onClose: () => void
  onRefresh: () => Promise<void>
  onOpen: (path: string) => void
  onViewRevision: (reference: string) => Promise<void>
  onReturnToWorktree: () => void
  readOnly?: boolean
}
export function GitPanel({
  open,
  path,
  git,
  loading,
  allowInit = true,
  onClose,
  onRefresh,
  onOpen,
  onViewRevision,
  onReturnToWorktree,
  readOnly,
}: Props) {
  const repo = useGitRepository(git?.root, path, open, onRefresh, onViewRevision)
  const [pickerOpen, setPickerOpen] = useState(false)
  const [commitOpen, setCommitOpen] = useState(false)
  useEffect(() => {
    setPickerOpen(false)
    setCommitOpen(false)
  }, [git?.root])
  const changes = useMemo<Change[]>(
    () =>
      git && !readOnly
        ? Object.entries(git.entries).flatMap(([path, code]) => {
            const state = gitState(code)
            return state ? [{ path, code, ...state }] : []
          })
        : [],
    [git, readOnly],
  )
  const staged = useMemo(() => changes.filter((c) => c.staged), [changes])
  const unstaged = useMemo(() => changes.filter((c) => c.unstaged), [changes])
  useEffect(() => {
    if (!open) return
    const handle = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.defaultPrevented && !pickerOpen && !commitOpen) onClose()
    }
    document.addEventListener('keydown', handle)
    return () => document.removeEventListener('keydown', handle)
  }, [open, onClose, pickerOpen, commitOpen])
  if (!open) return null
  const disabled = Boolean(repo.busy || loading)
  const commitReason =
    repo.busy ||
    (loading
      ? '正在读取修改'
      : !staged.length
        ? changes.length
          ? '先暂存要提交的文件'
          : '没有待提交的修改'
        : '')
  const sections = [
    { title: '已暂存', items: staged, staged: true },
    { title: '更改', items: unstaged, staged: false },
  ]
  return (
    <aside className="git-panel" aria-label="源代码管理" aria-busy={Boolean(repo.busy)}>
      <header>
        <span className="git-panel-title">
          <GitFork size={16} />
          源代码管理
        </span>
        <button
          className="icon-button"
          title="刷新源代码管理"
          aria-label="刷新源代码管理"
          disabled={disabled}
          onClick={() => void repo.refresh()}
        >
          <RefreshCw size={14} className={repo.busy === '正在刷新' ? 'spin' : ''} />
        </button>
        <button className="icon-button" title="关闭" aria-label="关闭源代码管理" onClick={onClose}>
          <X size={16} />
        </button>
      </header>
      {repo.error && (
        <div className="git-panel-error" role="alert">
          {repo.error}
        </div>
      )}
      {repo.success && (
        <div className="git-success" role="status">
          <CheckCircle2 size={14} />
          {repo.success}
        </div>
      )}
      {repo.busy && (
        <div className="git-progress" role="status">
          <LoaderCircle size={13} className="spin" />
          {repo.busy}…
        </div>
      )}
      {!git ? (
        <div className="git-welcome">
          <span className="git-welcome-icon">
            <FolderGit2 size={32} />
          </span>
          <Button disabled={disabled || !path || !allowInit} onClick={() => void repo.initialize()}>
            <Plus size={14} />
            {loading ? '正在检查仓库' : allowInit ? '初始化 Git 仓库' : '请选择本地文件夹'}
          </Button>
        </div>
      ) : (
        <>
          <div className="git-repository">
            <span className="git-repo-icon">
              <FolderGit2 size={20} />
            </span>
            <span className="git-repo-name">
              <strong>{basename(git.root)}</strong>
              <small title={git.root}>{git.root}</small>
            </span>
            <button
              className="git-branch-trigger"
              aria-label="选择 commit"
              disabled={disabled}
              onClick={() => setPickerOpen(true)}
            >
              <GitBranch size={13} />
              <span>
                {repo.refs?.current || git.branch} ·{' '}
                {readOnly ? git.branch : repo.refs?.head || '尚无 commit'}
              </span>
              <ChevronDown size={12} />
            </button>
          </div>
          <CommitPicker
            open={pickerOpen}
            onClose={() => setPickerOpen(false)}
            commits={repo.commits}
            path={path}
            busy={disabled}
            onSelect={async (id) => {
              await repo.browse(id)
              setPickerOpen(false)
            }}
            onMore={repo.moreHistory}
          />
          <CommitDialog
            open={commitOpen}
            onClose={() => setCommitOpen(false)}
            repo={repo}
            count={staged.length}
          />
          {readOnly && (
            <div className="git-history-return">
              <span>历史版本只读 · {git.branch}</span>
              <button onClick={onReturnToWorktree}>返回工作区</button>
            </div>
          )}
          {!readOnly ? (
            <>
              <GitRemote repo={repo} disabled={disabled} />
              <div className="git-commit-action">
                <Button disabled={disabled || !staged.length} onClick={() => setCommitOpen(true)}>
                  <GitCommitHorizontal size={15} />
                  {commitReason || `提交修改 · ${staged.length}`}
                </Button>
              </div>
              <div className="git-panel-scroll">
                {!changes.length && (
                  <div className="git-clean">
                    <CheckCircle2 size={28} />
                    <strong>工作区干净</strong>
                  </div>
                )}
                {sections
                  .filter((section) => section.items.length)
                  .map((section) => (
                    <section key={section.title}>
                      <h2>
                        <span>{section.title}</span>
                        <small>{section.items.length}</small>
                        <button
                          className="git-section-action"
                          disabled={disabled}
                          onClick={() =>
                            void (section.staged ? repo.unstage([], true) : repo.stage([], true))
                          }
                        >
                          {section.staged ? <Minus size={12} /> : <Plus size={12} />}
                          {section.staged ? '全部取消暂存' : '暂存全部'}
                        </button>
                      </h2>
                      {section.items.map((change) => (
                        <div className="git-change-row" key={change.path}>
                          <button
                            className="git-change-open"
                            title={change.path}
                            onClick={() => onOpen(change.path)}
                          >
                            <FileCode2 size={15} />
                            <span className="git-change-name">
                              <span className={`git-file git-file-${change.letter.toLowerCase()}`}>
                                {basename(change.path)}
                              </span>
                              <small>{change.path.slice(git.root.length + 1)}</small>
                            </span>
                          </button>
                          <span
                            className={`git-change-badge git-file-${change.letter.toLowerCase()}`}
                            title={change.label}
                          >
                            {change.code === '??'
                              ? 'U'
                              : section.staged
                                ? change.code[0]
                                : change.code[1]}
                          </span>
                          <button
                            className="git-file-action"
                            aria-label={`${section.staged ? '取消暂存' : '暂存'} ${basename(change.path)}`}
                            disabled={disabled}
                            title={section.staged ? '取消暂存' : '暂存'}
                            onClick={() =>
                              void (section.staged
                                ? repo.unstage([change.path])
                                : repo.stage([change.path]))
                            }
                          >
                            {section.staged ? <Minus size={14} /> : <Plus size={14} />}
                          </button>
                        </div>
                      ))}
                    </section>
                  ))}
              </div>
            </>
          ) : null}
        </>
      )}
    </aside>
  )
}

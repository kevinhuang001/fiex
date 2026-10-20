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
  UserRound,
  X,
} from 'lucide-react'
import { basename } from '@/lib/files'
import { gitState } from '@/lib/git'
import { useGitRepository } from '@/hooks/useGitRepository'
import type { GitStatus } from '@/lib/types'
import { Button } from './ui/button'
import * as D from './ui/dropdown-menu'

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
  const [section, setSection] = useState<'changes' | 'history'>('changes')
  const [identityOpen, setIdentityOpen] = useState(true)
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
      if (event.key === 'Escape' && !event.defaultPrevented) onClose()
    }
    document.addEventListener('keydown', handle)
    return () => document.removeEventListener('keydown', handle)
  }, [open, onClose])
  if (!open) return null
  const disabled = Boolean(repo.busy || loading)
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
          <h2>{loading ? '正在检查仓库' : '为这个文件夹记录每一次改变'}</h2>
          <p>
            {allowInit
              ? '创建 Git 仓库，暂存文件并保存提交。文件内容会保留在原处。'
              : '请选择本地文件夹来使用 Git。'}
          </p>
          <code title={path}>{path}</code>
          <Button disabled={disabled || !path || !allowInit} onClick={() => void repo.initialize()}>
            <Plus size={14} />
            初始化 Git 仓库
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
            <D.DropdownMenu>
              <D.DropdownMenuTrigger asChild>
                <button
                  className="git-branch-trigger"
                  aria-label="切换分支或版本"
                  disabled={disabled}
                >
                  <GitBranch size={13} />
                  <span>{git.branch}</span>
                  <ChevronDown size={12} />
                </button>
              </D.DropdownMenuTrigger>
              <D.DropdownMenuContent align="end" className="fiex-menu git-ref-menu">
                <D.DropdownMenuLabel>分支</D.DropdownMenuLabel>
                <D.DropdownMenuRadioGroup
                  value={git.branch}
                  onValueChange={(ref) => {
                    setSection('history')
                    void repo.browse(ref)
                  }}
                >
                  {(repo.refs?.branches.length ? repo.refs.branches : [git.branch]).map(
                    (branch) => (
                      <D.DropdownMenuRadioItem value={branch} key={branch}>
                        {branch}
                      </D.DropdownMenuRadioItem>
                    ),
                  )}
                  {!!repo.refs?.tags.length && <D.DropdownMenuLabel>标签</D.DropdownMenuLabel>}
                  {repo.refs?.tags.map((tag) => (
                    <D.DropdownMenuRadioItem value={tag} key={'tag:' + tag}>
                      {tag}
                    </D.DropdownMenuRadioItem>
                  ))}
                </D.DropdownMenuRadioGroup>
              </D.DropdownMenuContent>
            </D.DropdownMenu>
          </div>
          <div className="git-section-tabs">
            <button
              aria-pressed={!readOnly && section === 'changes'}
              disabled={readOnly}
              onClick={() => setSection('changes')}
            >
              更改
            </button>
            <button
              aria-pressed={readOnly || section === 'history'}
              onClick={() => setSection('history')}
            >
              提交历史
            </button>
          </div>
          {readOnly && (
            <div className="git-history-return">
              <span>历史版本只读 · {git.branch}</span>
              <button onClick={onReturnToWorktree}>返回工作区</button>
            </div>
          )}
          {(readOnly || section === 'history') && (
            <div className="git-history-list">
              {!repo.commits.length && (
                <p className="git-panel-empty">暂无提交记录，完成首次提交后会显示在这里。</p>
              )}
              {repo.commits.map((commit) => (
                <button
                  key={commit.id}
                  className={`git-history-item ${path.includes(commit.id) ? 'active' : ''}`}
                  aria-label={`查看提交 ${commit.shortId}`}
                  disabled={disabled}
                  onClick={() => void repo.browse(commit.id)}
                >
                  <span className="git-history-marker">
                    <GitCommitHorizontal size={16} />
                  </span>
                  <span>
                    <strong>{commit.subject}</strong>
                    <small>
                      {commit.author} ·{' '}
                      {new Date(commit.timestamp * 1000).toLocaleDateString('zh-CN')}
                    </small>
                    <code>{commit.shortId}</code>
                  </span>
                </button>
              ))}
              {!!repo.commits.length && repo.commits.length % 50 === 0 && (
                <button
                  className="git-load-more"
                  disabled={disabled}
                  onClick={() => void repo.moreHistory()}
                >
                  加载更早的提交
                </button>
              )}
            </div>
          )}
          {!readOnly && section === 'changes' ? (
            <>
              <form
                className="git-commit-form"
                onSubmit={(event) => {
                  event.preventDefault()
                  if (!disabled && staged.length && repo.message.trim()) void repo.commit()
                }}
              >
                <label className="visually-hidden" htmlFor="git-message">
                  提交说明
                </label>
                <textarea
                  id="git-message"
                  aria-label="提交说明"
                  placeholder="这次做了哪些改动？"
                  value={repo.message}
                  disabled={disabled}
                  onChange={(e) => repo.setMessage(e.target.value)}
                  rows={3}
                  onKeyDown={(event) => {
                    if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
                      event.preventDefault()
                      event.currentTarget.form?.requestSubmit()
                    }
                  }}
                />
                <details
                  className="git-identity"
                  open={identityOpen}
                  onToggle={(e) => setIdentityOpen(e.currentTarget.open)}
                >
                  <summary>
                    <UserRound size={12} />
                    {repo.identity.name || '设置提交身份'}
                    <ChevronDown size={12} />
                  </summary>
                  <div>
                    <input
                      aria-label="提交者姓名"
                      placeholder="姓名"
                      value={repo.identity.name}
                      disabled={disabled}
                      onChange={(e) => repo.setIdentity({ ...repo.identity, name: e.target.value })}
                    />
                    <input
                      aria-label="提交者邮箱"
                      placeholder="邮箱"
                      type="email"
                      value={repo.identity.email}
                      disabled={disabled}
                      onChange={(e) =>
                        repo.setIdentity({ ...repo.identity, email: e.target.value })
                      }
                    />
                  </div>
                  <small>仅用于此提交，不修改全局 Git 配置。</small>
                </details>
                <Button
                  type="submit"
                  disabled={
                    disabled ||
                    !staged.length ||
                    !repo.message.trim() ||
                    !repo.identity.name.trim() ||
                    !repo.identity.email.trim()
                  }
                >
                  <GitCommitHorizontal size={15} />
                  {staged.length ? `提交 ${staged.length} 个文件` : '提交更改'}
                </Button>
                <span className="git-commit-hint">
                  {staged.length
                    ? 'Ctrl / ⌘ + Enter 提交'
                    : changes.length
                      ? '先暂存要提交的文件'
                      : '工作区没有待提交的更改'}
                </span>
              </form>
              <div className="git-panel-scroll">
                {!changes.length && (
                  <div className="git-clean">
                    <CheckCircle2 size={28} />
                    <strong>工作区干净</strong>
                    <span>所有改动都已保存到提交。</span>
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

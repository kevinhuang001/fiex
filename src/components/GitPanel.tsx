import { useEffect, useState } from 'react'
import { GitBranch, RefreshCw, X } from 'lucide-react'
import { call } from '@/lib/api'
import { basename } from '@/lib/files'
import { gitState } from '@/lib/git'
import type { GitRefs, GitStatus } from '@/lib/types'

interface Change {
  path: string
  code: string
  letter: string
  label: string
  staged: boolean
  unstaged: boolean
}

function toChanges(git: GitStatus): Change[] {
  return Object.entries(git.entries).flatMap(([path, code]) => {
    const state = gitState(code)
    return state
      ? [
          {
            path,
            code,
            letter: state.letter,
            label: state.label,
            staged: state.staged,
            unstaged: state.unstaged,
          },
        ]
      : []
  })
}

function dirOf(path: string, root: string) {
  const relative = path.startsWith(root + '/') ? path.slice(root.length + 1) : path
  const parts = relative.split('/')
  parts.pop()
  return parts.join('/')
}

export function GitPanel({
  open,
  git,
  onClose,
  onRefresh,
  onOpen,
  readOnly,
}: {
  open: boolean
  git: GitStatus | null
  onClose: () => void
  onRefresh: () => void
  onOpen: (path: string) => void
  readOnly?: boolean
}) {
  const [refs, setRefs] = useState<GitRefs | null>(null)
  const [selected, setSelected] = useState('')
  const [switching, setSwitching] = useState(false)
  const [branchError, setBranchError] = useState('')

  useEffect(() => {
    if (!open || !git) return
    let active = true
    void call<GitRefs>('git_refs', { root: git.root })
      .then((next) => {
        if (active) {
          setRefs(next)
          setSelected(next.current)
        }
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [open, git?.root])

  useEffect(() => {
    if (!open) return
    const close = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', close)
    return () => document.removeEventListener('keydown', close)
  }, [open, onClose])

  if (!open) return null

  const changes = git && !readOnly ? toChanges(git) : []
  const staged = changes.filter((change) => change.staged)
  const unstaged = changes.filter((change) => change.code !== '??' && change.unstaged)
  const untracked = changes.filter((change) => change.code === '??')

  const row = (change: Change) => (
    <button
      className="git-change-row"
      key={`${change.code}:${change.path}`}
      title={change.path}
      onClick={() => onOpen(change.path)}
    >
      <span className="git-change-name">
        <span className={`git-file git-file-${change.letter.toLowerCase()}`}>
          {basename(change.path)}
        </span>
        <small>{dirOf(change.path, git?.root || '')}</small>
      </span>
      <span className="git-change-label">{change.label}</span>
    </button>
  )

  return (
    <aside className="git-panel" aria-label="源代码管理">
      <header>
        <span className="git-panel-title">
          <GitBranch size={14} />
          源代码管理
        </span>
        <button className="icon-button" title="刷新" onClick={onRefresh}>
          <RefreshCw size={13} />
        </button>
        <button className="icon-button" title="关闭" aria-label="关闭源代码管理" onClick={onClose}>
          <X size={14} />
        </button>
      </header>
      {!git ? (
        <p className="git-panel-empty">当前文件夹不在 Git 仓库中。</p>
      ) : (
        <>
          <div className="git-panel-branch">
            <select
              className="git-branch-select"
              value={selected}
              disabled={switching}
              aria-label="切换分支或版本"
              onChange={async (event) => {
                const next = event.target.value
                setSelected(next)
                setSwitching(true)
                setBranchError('')
                try {
                  await call('git_checkout', { root: git.root, reference: next })
                  onRefresh()
                  const fresh = await call<GitRefs>('git_refs', { root: git.root })
                  setRefs(fresh)
                  setSelected(fresh.current)
                } catch (error) {
                  setBranchError(String(error))
                  setSelected(git.branch)
                } finally {
                  setSwitching(false)
                }
              }}
            >
              {refs?.branches.map((branch) => (
                <option key={`branch:${branch}`} value={branch}>
                  {branch}
                </option>
              ))}
              {refs?.tags.map((tag) => (
                <option key={`tag:${tag}`} value={tag}>
                  标签 · {tag}
                </option>
              ))}
              {!refs?.branches.includes(selected) && !refs?.tags.includes(selected) && (
                <option value={selected}>{selected || git.branch}</option>
              )}
            </select>
            <small>{readOnly ? '只读' : `${changes.length} 个更改`}</small>
          </div>
          {branchError && <p className="git-panel-error">{branchError}</p>}
          <div className="git-panel-scroll">
            {readOnly ? (
              <p className="git-readonly-note">只读查看分支或版本，不显示工作区更改</p>
            ) : (
              changes.length === 0 && <p className="git-panel-empty">没有更改。</p>
            )}
            {staged.length > 0 && (
              <section>
                <h2>
                  暂存的更改 <small>{staged.length}</small>
                </h2>
                {staged.map((change) => row(change))}
              </section>
            )}
            {unstaged.length > 0 && (
              <section>
                <h2>
                  更改 <small>{unstaged.length}</small>
                </h2>
                {unstaged.map((change) => row(change))}
              </section>
            )}
            {untracked.length > 0 && (
              <section>
                <h2>
                  未跟踪 <small>{untracked.length}</small>
                </h2>
                {untracked.map((change) => row(change))}
              </section>
            )}
          </div>
        </>
      )}
    </aside>
  )
}

import { useEffect, useState } from 'react'
import { Check, GitBranch, LoaderCircle, Minus, Plus, RefreshCw, X } from 'lucide-react'
import { call } from '@/lib/api'
import { basename } from '@/lib/files'
import { gitState } from '@/lib/git'
import type { GitStatus } from '@/lib/types'

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
}: {
  open: boolean
  git: GitStatus | null
  onClose: () => void
  onRefresh: () => void
  onOpen: (path: string) => void
}) {
  const [message, setMessage] = useState('')
  const [busy, setBusy] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    if (!open) return
    const close = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', close)
    return () => document.removeEventListener('keydown', close)
  }, [open, onClose])

  if (!open) return null

  const changes = git ? toChanges(git) : []
  const staged = changes.filter((change) => change.staged)
  const unstaged = changes.filter((change) => change.code !== '??' && change.unstaged)
  const untracked = changes.filter((change) => change.code === '??')

  const run = async (label: string, fn: () => Promise<unknown>) => {
    setBusy(label)
    setError('')
    try {
      await fn()
      onRefresh()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy('')
    }
  }

  const stage = (path: string, unstage = false) => {
    if (!git) return
    void run(path, () =>
      call(unstage ? 'git_unstage' : 'git_stage', { root: git.root, paths: [path] }),
    )
  }

  const commit = () => {
    if (!git || !message.trim() || !staged.length) return
    void run('commit', async () => {
      await call('git_commit', { root: git.root, message })
      setMessage('')
    })
  }

  const row = (change: Change, canStage: boolean) => (
    <div className="git-change-row" key={`${change.code}:${change.path}`}>
      <button className="git-change-open" title={change.path} onClick={() => onOpen(change.path)}>
        <span className={`git-badge git-${change.letter.toLowerCase()}`}>{change.letter}</span>
        <span className="git-change-name">
          <span className={`git-file git-file-${change.letter.toLowerCase()}`}>
            {basename(change.path)}
          </span>
          <small>{dirOf(change.path, git?.root || '')}</small>
        </span>
        <span className="git-change-label">{change.label}</span>
      </button>
      <button
        className="git-change-action"
        title={canStage ? '暂存' : '取消暂存'}
        aria-label={canStage ? '暂存' : '取消暂存'}
        disabled={Boolean(busy)}
        onClick={() => stage(change.path, !canStage)}
      >
        {canStage ? <Plus size={12} /> : <Minus size={12} />}
      </button>
    </div>
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
            <span>{git.branch}</span>
            <small>{changes.length} 个更改</small>
          </div>
          <div className="git-commit-box">
            <input
              value={message}
              placeholder="提交信息"
              onChange={(event) => setMessage(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') commit()
              }}
            />
            <button
              className="git-commit-button"
              disabled={!message.trim() || !staged.length || Boolean(busy)}
              onClick={commit}
              title="提交暂存的更改"
            >
              {busy === 'commit' ? (
                <LoaderCircle className="spin" size={13} />
              ) : (
                <Check size={13} />
              )}
              提交
            </button>
          </div>
          {error && <p className="git-panel-error">{error}</p>}
          <div className="git-panel-scroll">
            {changes.length === 0 && <p className="git-panel-empty">没有更改。</p>}
            {staged.length > 0 && (
              <section>
                <h2>
                  暂存的更改 <small>{staged.length}</small>
                </h2>
                {staged.map((change) => row(change, false))}
              </section>
            )}
            {unstaged.length > 0 && (
              <section>
                <h2>
                  更改 <small>{unstaged.length}</small>
                </h2>
                {unstaged.map((change) => row(change, true))}
              </section>
            )}
            {untracked.length > 0 && (
              <section>
                <h2>
                  未跟踪 <small>{untracked.length}</small>
                </h2>
                {untracked.map((change) => row(change, true))}
              </section>
            )}
          </div>
        </>
      )}
    </aside>
  )
}

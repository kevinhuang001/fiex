import { useEffect, useState } from 'react'
import { ArrowDown, ArrowUp, Check, Cloud, Pencil, Plus, Trash2 } from 'lucide-react'
import type { useGitRepository } from '@/hooks/useGitRepository'
import { Button } from '../ui/button'
import { Modal } from '../ui/Modal'

/// The remote manager lists what the repository already has (origin is not
/// special) and switches to a small form when adding or editing, so the list
/// stays a read-only overview with the sync controls underneath.
export function GitRemoteModal({
  repo,
  disabled,
  open,
  onClose,
}: {
  repo: ReturnType<typeof useGitRepository>
  disabled: boolean
  open: boolean
  onClose: () => void
}) {
  const [editing, setEditing] = useState<{
    mode: 'new' | 'edit'
    name: string
    url: string
  } | null>(null)
  const [pendingRemove, setPendingRemove] = useState<string | null>(null)
  const [branch, setBranch] = useState('')
  const status = repo.remoteStatus
  const remotes = status?.remotes || []
  const selected = remotes.find((remote) => remote.name === repo.remote)
  const busy = Boolean(repo.busy) || disabled
  useEffect(() => {
    if (!open) return
    setEditing(null)
    setPendingRemove(null)
    setBranch(repo.refs?.current || '')
  }, [open, repo.refs?.current])
  return (
    <Modal
      open={open}
      onClose={() => {
        if (!repo.busy) onClose()
      }}
      title={editing ? (editing.mode === 'new' ? '添加远程仓库' : '编辑远程仓库') : '远程仓库'}
      className="git-remote-modal"
    >
      {editing ? (
        <RemoteForm
          repo={repo}
          busy={busy}
          editing={editing}
          onCancel={() => setEditing(null)}
          onSaved={() => setEditing(null)}
        />
      ) : (
        <>
          <section className="git-remote-section">
            <div className="git-remote-section-title">
              <span>已配置</span>
              <span className="git-remote-badge">{remotes.length}</span>
              <span className="git-remote-count" title={status?.upstream || ''}>
                ↓ {status?.behind || 0} · ↑ {status?.ahead || 0}
              </span>
            </div>
            {remotes.length ? (
              <ul className="git-remote-list" role="listbox" aria-label="远程仓库">
                {remotes.map((remote) => (
                  <li
                    key={remote.name}
                    className={`git-remote-row ${remote.name === repo.remote ? 'active' : ''}`}
                  >
                    {pendingRemove === remote.name ? (
                      <div className="git-remote-confirm">
                        <span>移除 {remote.name}？</span>
                        <Button
                          variant="ghost"
                          disabled={busy}
                          onClick={() => setPendingRemove(null)}
                        >
                          取消
                        </Button>
                        <Button
                          variant="destructive"
                          disabled={busy}
                          onClick={() => {
                            void repo.removeRemote(remote.name)
                            setPendingRemove(null)
                          }}
                        >
                          移除
                        </Button>
                      </div>
                    ) : (
                      <>
                        <button
                          role="option"
                          aria-selected={remote.name === repo.remote}
                          className="git-remote-select"
                          onClick={() => repo.setRemote(remote.name)}
                        >
                          <Cloud size={14} />
                          <span className="git-remote-name">{remote.name}</span>
                          <span className="git-remote-url" title={remote.url}>
                            {remote.url}
                          </span>
                          {remote.name === repo.remote && (
                            <Check size={13} className="git-remote-check" />
                          )}
                        </button>
                        <button
                          className="icon-button"
                          title={`编辑 ${remote.name}`}
                          aria-label={`编辑 ${remote.name}`}
                          disabled={busy}
                          onClick={() =>
                            setEditing({ mode: 'edit', name: remote.name, url: remote.url })
                          }
                        >
                          <Pencil size={13} />
                        </button>
                        <button
                          className="icon-button"
                          title={`移除 ${remote.name}`}
                          aria-label={`移除 ${remote.name}`}
                          disabled={busy}
                          onClick={() => setPendingRemove(remote.name)}
                        >
                          <Trash2 size={13} />
                        </button>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            ) : (
              <p className="git-remote-empty">无远程仓库</p>
            )}
            <button
              className="git-remote-add"
              disabled={busy}
              onClick={() => setEditing({ mode: 'new', name: '', url: '' })}
            >
              <Plus size={13} />
              添加远程仓库
            </button>
          </section>
          <section className="git-remote-sync">
            <label className="git-remote-branch">
              分支
              <select
                className="text-field"
                aria-label="同步分支"
                value={branch || repo.refs?.current || ''}
                onChange={(e) => setBranch(e.target.value)}
              >
                {(repo.refs?.branches.length
                  ? repo.refs.branches
                  : repo.refs?.current
                    ? [repo.refs.current]
                    : []
                ).map((item) => (
                  <option key={item} value={item}>
                    {item}
                  </option>
                ))}
              </select>
            </label>
            <Button
              type="button"
              variant="secondary"
              disabled={busy || !selected}
              onClick={() => void repo.sync(false, branch)}
            >
              <ArrowDown size={14} />
              拉取
            </Button>
            <Button
              type="button"
              variant="secondary"
              disabled={busy || !selected || !repo.refs?.head}
              onClick={() => void repo.sync(true, branch)}
            >
              <ArrowUp size={14} />
              推送
            </Button>
          </section>
          {repo.error && (
            <p className="git-panel-error git-remote-error" role="alert">
              {repo.error}
            </p>
          )}
        </>
      )}
    </Modal>
  )
}

function RemoteForm({
  repo,
  busy,
  editing,
  onCancel,
  onSaved,
}: {
  repo: ReturnType<typeof useGitRepository>
  busy: boolean
  editing: { mode: 'new' | 'edit'; name: string; url: string }
  onCancel: () => void
  onSaved: () => void
}) {
  const [name, setName] = useState(editing.name)
  const [url, setUrl] = useState(editing.url)
  return (
    <form
      className="git-remote-form"
      onSubmit={async (e) => {
        e.preventDefault()
        if (await repo.saveRemote(name, url)) onSaved()
      }}
    >
      <label>
        名称
        <input
          className="text-field"
          aria-label="远程名称"
          value={name}
          required
          placeholder="origin"
          onChange={(e) => setName(e.target.value)}
        />
      </label>
      <label>
        仓库地址
        <input
          className="text-field"
          aria-label="仓库地址"
          value={url}
          required
          placeholder="https://… / git@…"
          onChange={(e) => setUrl(e.target.value)}
        />
      </label>
      {repo.error && (
        <p className="git-panel-error" role="alert">
          {repo.error}
        </p>
      )}
      <div className="git-remote-actions">
        <Button type="button" variant="ghost" disabled={Boolean(repo.busy)} onClick={onCancel}>
          返回
        </Button>
        <Button type="submit" disabled={busy || !name.trim() || !url.trim()}>
          {repo.busy || '保存'}
        </Button>
      </div>
    </form>
  )
}

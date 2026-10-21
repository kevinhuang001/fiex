import { useEffect, useState } from 'react'
import { ArrowDown, ArrowUp, Check, Cloud } from 'lucide-react'
import type { useGitRepository } from '@/hooks/useGitRepository'
import { Button } from '../ui/button'
import { Modal } from '../ui/Modal'

/// Everything about remotes lives here: selection, name/address editing and
/// the sync actions. The panel itself only keeps changes and the commit box.
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
  const [name, setName] = useState('origin')
  const [url, setUrl] = useState('')
  const status = repo.remoteStatus
  const remotes = status?.remotes || []
  const configured = remotes.length > 0
  const busy = Boolean(repo.busy) || disabled
  useEffect(() => {
    if (!open) return
    const current = remotes.find((r) => r.name === repo.remote) || remotes[0]
    setName(current?.name || 'origin')
    setUrl(current?.url || '')
  }, [open, repo.remote])
  const choose = (name: string) => {
    const remote = remotes.find((r) => r.name === name)
    repo.setRemote(name)
    setName(remote?.name || name)
    setUrl(remote?.url || '')
  }
  return (
    <Modal
      open={open}
      onClose={() => {
        if (!repo.busy) onClose()
      }}
      title="远程仓库"
      className="git-remote-modal"
    >
      {configured && (
        <div className="git-remote-list" role="listbox" aria-label="已有远程仓库">
          {remotes.map((remote) => (
            <button
              key={remote.name}
              role="option"
              aria-selected={remote.name === repo.remote}
              className={`git-remote-item ${remote.name === repo.remote ? 'active' : ''}`}
              onClick={() => choose(remote.name)}
            >
              <Cloud size={13} />
              {remote.name}
              {remote.name === repo.remote && <Check size={13} className="git-remote-check" />}
            </button>
          ))}
          <span className="git-remote-count" title={status?.upstream || ''}>
            ↓ {status?.behind || 0} · ↑ {status?.ahead || 0}
          </span>
        </div>
      )}
      <form
        className="git-commit-form"
        onSubmit={async (e) => {
          e.preventDefault()
          if (await repo.saveRemote(name, url)) onClose()
        }}
      >
        <label>
          名称
          <input
            className="text-field"
            aria-label="远程名称"
            value={name}
            required
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
        <div className="dialog-actions">
          <Button
            type="button"
            variant="secondary"
            disabled={busy || !configured}
            onClick={() => void repo.sync(false)}
          >
            <ArrowDown size={14} />
            拉取
          </Button>
          <Button
            type="button"
            variant="secondary"
            disabled={busy || !configured || !repo.refs?.head}
            onClick={() => void repo.sync(true)}
          >
            <ArrowUp size={14} />
            推送
          </Button>
          <Button type="submit" disabled={busy || !name.trim() || !url.trim()}>
            {repo.busy || '保存'}
          </Button>
        </div>
      </form>
    </Modal>
  )
}

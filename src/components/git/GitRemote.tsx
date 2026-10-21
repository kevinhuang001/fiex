import { useEffect, useState } from 'react'
import { ArrowDown, ArrowUp, ChevronDown, Cloud } from 'lucide-react'
import type { useGitRepository } from '@/hooks/useGitRepository'
import { Choice, Menu } from '../Menus'
import { Button } from '../ui/button'
import { Modal } from '../ui/Modal'

/// Remote configuration lives behind the header button so the panel keeps its
/// space for changes, commits and the sync counters.
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
  useEffect(() => {
    if (!open) return
    const current = repo.remoteStatus?.remotes.find((r) => r.name === repo.remote)
    setName(current?.name || 'origin')
    setUrl(current?.url || '')
  }, [open, repo.remote, repo.remoteStatus])
  return (
    <Modal
      open={open}
      onClose={() => {
        if (!repo.busy) onClose()
      }}
      title="远程仓库"
      className="git-remote-modal"
    >
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
            type="submit"
            disabled={disabled || Boolean(repo.busy) || !name.trim() || !url.trim()}
          >
            {repo.busy || '保存'}
          </Button>
        </div>
      </form>
    </Modal>
  )
}

export function GitRemoteBar({
  repo,
  disabled,
}: {
  repo: ReturnType<typeof useGitRepository>
  disabled: boolean
}) {
  const status = repo.remoteStatus
  const configured = Boolean(status?.remotes.length)
  return (
    <div className="git-remote">
      <div className="git-remote-heading">
        <Cloud size={14} />
        <span>{configured ? repo.remote : '未配置远程仓库'}</span>
        {configured && (
          <Menu icon={<ChevronDown size={11} />} label="选择远程仓库">
            {status?.remotes.map((r) => (
              <Choice
                key={r.name}
                checked={r.name === repo.remote}
                onClick={() => repo.setRemote(r.name)}
              >
                {r.name}
              </Choice>
            ))}
          </Menu>
        )}
        <span className="git-remote-count" title={status?.upstream || ''}>
          ↓ {status?.behind || 0} · ↑ {status?.ahead || 0}
        </span>
      </div>
      <div className="git-remote-actions">
        <Button
          variant="secondary"
          disabled={disabled || !configured}
          onClick={() => void repo.sync(false)}
        >
          <ArrowDown size={14} />
          拉取
        </Button>
        <Button
          variant="secondary"
          disabled={disabled || !configured || !repo.refs?.head}
          onClick={() => void repo.sync(true)}
        >
          <ArrowUp size={14} />
          推送
        </Button>
      </div>
    </div>
  )
}

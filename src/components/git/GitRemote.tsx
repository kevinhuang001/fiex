import { useState } from 'react'
import { ArrowDown, ArrowUp, ChevronDown, Cloud, Settings2 } from 'lucide-react'
import type { useGitRepository } from '@/hooks/useGitRepository'
import { Choice, Menu } from '../Menus'
import { Button } from '../ui/button'
import { Modal } from '../ui/Modal'

export function GitRemote({
  repo,
  disabled,
}: {
  repo: ReturnType<typeof useGitRepository>
  disabled: boolean
}) {
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('origin')
  const [url, setUrl] = useState('')
  const status = repo.remoteStatus
  const unavailable = !status?.remotes.length
    ? '配置远程仓库'
    : !status.branch
      ? '请选择本地分支'
      : ''
  const configure = () => {
    const current = status?.remotes.find((r) => r.name === repo.remote)
    setName(current?.name || 'origin')
    setUrl(current?.url || '')
    setOpen(true)
  }
  return (
    <>
      <div className="git-remote">
        <div className="git-remote-heading">
          <Cloud size={14} />
          {status?.remotes.length ? (
            <>
              <span>{repo.remote}</span>
              <Menu icon={<ChevronDown size={11} />} label="选择远程仓库">
                {status.remotes.map((r) => (
                  <Choice
                    key={r.name}
                    checked={r.name === repo.remote}
                    onClick={() => repo.setRemote(r.name)}
                  >
                    {r.name}
                  </Choice>
                ))}
              </Menu>
            </>
          ) : (
            <span>远程仓库</span>
          )}
          <span className="git-remote-count" title={status?.upstream || ''}>
            ↓ {status?.behind || 0} · ↑ {status?.ahead || 0}
          </span>
          <button
            className="icon-button"
            aria-label="配置远程仓库"
            title="配置远程仓库"
            disabled={disabled}
            onClick={configure}
          >
            <Settings2 size={14} />
          </button>
        </div>
        {unavailable ? (
          <Button
            variant="secondary"
            disabled={disabled || Boolean(status?.remotes.length)}
            onClick={configure}
          >
            {unavailable}
          </Button>
        ) : (
          <div className="git-remote-actions">
            <Button variant="secondary" disabled={disabled} onClick={() => void repo.sync(false)}>
              <ArrowDown size={14} />
              拉取
            </Button>
            <Button
              variant="secondary"
              disabled={disabled || !repo.refs?.head}
              onClick={() => void repo.sync(true)}
            >
              <ArrowUp size={14} />
              推送
            </Button>
          </div>
        )}
      </div>
      <Modal
        open={open}
        onClose={() => {
          if (!repo.busy) setOpen(false)
        }}
        title="远程仓库"
        className="git-remote-modal"
      >
        <form
          className="git-commit-form"
          onSubmit={async (e) => {
            e.preventDefault()
            if (await repo.saveRemote(name, url)) setOpen(false)
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
            <Button type="submit" disabled={Boolean(repo.busy) || !name.trim() || !url.trim()}>
              {repo.busy || '保存'}
            </Button>
          </div>
        </form>
      </Modal>
    </>
  )
}

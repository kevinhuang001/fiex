import { GitCommitHorizontal } from 'lucide-react'
import type { useGitRepository } from '@/hooks/useGitRepository'
import { Button } from '../ui/button'
import { Modal } from '../ui/Modal'

export function CommitDialog({
  open,
  onClose,
  repo,
  count,
}: {
  open: boolean
  onClose: () => void
  repo: ReturnType<typeof useGitRepository>
  count: number
}) {
  const missing = !repo.identity.name.trim() || !repo.identity.email.trim()
  return (
    <Modal
      open={open}
      onClose={() => {
        if (!repo.busy) onClose()
      }}
      title="提交修改"
      description={`将 ${count} 个已暂存文件保存到当前分支。`}
      className="git-commit-modal"
    >
      <form
        className="git-commit-form"
        onSubmit={async (e) => {
          e.preventDefault()
          if (!repo.busy && count && !missing && repo.message.trim() && (await repo.commit()))
            onClose()
        }}
      >
        <label htmlFor="git-message">提交说明 / comment</label>
        <textarea
          id="git-message"
          aria-label="提交说明"
          placeholder="这次做了哪些改动？"
          value={repo.message}
          required
          disabled={Boolean(repo.busy)}
          onChange={(e) => repo.setMessage(e.target.value)}
          rows={4}
          onKeyDown={(e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
              e.preventDefault()
              e.currentTarget.form?.requestSubmit()
            }
          }}
        />
        <fieldset className="git-identity" disabled={Boolean(repo.busy)}>
          <legend>提交身份</legend>
          <div>
            <label>
              姓名
              <input
                aria-label="提交者姓名"
                value={repo.identity.name}
                required
                placeholder="姓名"
                onChange={(e) => repo.setIdentity({ ...repo.identity, name: e.target.value })}
              />
            </label>
            <label>
              邮箱
              <input
                aria-label="提交者邮箱"
                type="email"
                value={repo.identity.email}
                required
                placeholder="you@example.com"
                onChange={(e) => repo.setIdentity({ ...repo.identity, email: e.target.value })}
              />
            </label>
          </div>
          <small>
            {missing ? '缺少 Git 身份，请填写姓名和邮箱。' : '默认使用当前项目 / 全局 Git 配置。'}
            提交时将保存到此项目的 Git 配置。
          </small>
        </fieldset>
        {repo.error && (
          <p className="git-panel-error" role="alert">
            {repo.error}
          </p>
        )}
        <Button
          type="submit"
          disabled={Boolean(repo.busy) || !count || missing || !repo.message.trim()}
        >
          <GitCommitHorizontal size={15} />
          {repo.busy || `提交 ${count} 个文件`}
        </Button>
        <span className="git-commit-hint">Ctrl / ⌘ + Enter 提交</span>
      </form>
    </Modal>
  )
}

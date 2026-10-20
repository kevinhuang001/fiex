import { useState } from 'react'
import { GitCommitHorizontal, Search } from 'lucide-react'
import type { GitCommit } from '@/lib/types'
import { Modal } from '../ui/Modal'

export function CommitPicker({
  open,
  onClose,
  commits,
  path,
  busy,
  onSelect,
  onMore,
}: {
  open: boolean
  onClose: () => void
  commits: GitCommit[]
  path: string
  busy: boolean
  onSelect: (id: string) => Promise<void>
  onMore: () => Promise<unknown>
}) {
  const [query, setQuery] = useState('')
  const search = query.trim().toLocaleLowerCase()
  const visible = commits.filter((c) =>
    `${c.id} ${c.subject} ${c.author} ${c.refs}`.toLocaleLowerCase().includes(search),
  )
  return (
    <Modal
      open={open}
      onClose={onClose}
      title="选择 commit"
      description="所有分支 · 按提交时间从新到旧 · 历史版本只读"
      className="git-picker-modal"
    >
      <label className="git-commit-search">
        <Search size={15} />
        <input
          aria-label="搜索 commit"
          placeholder="搜索说明、分支、作者或 SHA"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>
      <div className="git-history-list">
        {!visible.length && (
          <p className="git-panel-empty">{commits.length ? '没有匹配的提交' : '暂无提交记录'}</p>
        )}
        {visible.map((commit) => (
          <button
            key={commit.id}
            className={`git-history-item ${path.includes(commit.id) ? 'active' : ''}`}
            aria-label={`查看提交 ${commit.shortId}`}
            disabled={busy}
            onClick={() => void onSelect(commit.id)}
          >
            <span className="git-history-marker">
              <GitCommitHorizontal size={17} />
            </span>
            <span>
              <strong>{commit.subject}</strong>
              <small>
                {commit.author} · {new Date(commit.timestamp * 1000).toLocaleString('zh-CN')}
              </small>
              <code>
                {commit.shortId}
                {commit.refs && <span className="git-ref-label">{commit.refs}</span>}
              </code>
            </span>
          </button>
        ))}
        {!!commits.length && commits.length % 50 === 0 && (
          <button className="git-load-more" disabled={busy} onClick={() => void onMore()}>
            加载更早的提交
          </button>
        )}
      </div>
    </Modal>
  )
}

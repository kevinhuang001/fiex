import { useEffect, useMemo, useState } from 'react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import type { Entry } from '@/lib/types'

type Rename = { path: string; name: string }

function splitName(entry: Entry) {
  const dot = entry.isDir ? -1 : entry.name.lastIndexOf('.')
  return {
    stem: dot > 0 ? entry.name.slice(0, dot) : entry.name,
    ext: dot > 0 ? entry.name.slice(dot) : '',
  }
}

function expand(template: string, entry: Entry, index: number, find: string, replace: string) {
  const base = find ? entry.name.split(find).join(replace) : entry.name
  const { stem, ext } = splitName({ ...entry, name: base })
  return template
    .replaceAll('{name}', base)
    .replaceAll('{stem}', stem)
    .replaceAll('{ext}', ext)
    .replaceAll('{nnn}', String(index + 1).padStart(3, '0'))
    .replaceAll('{nn}', String(index + 1).padStart(2, '0'))
    .replaceAll('{n}', String(index + 1))
}

export function BatchRenameDialog({
  entries,
  onClose,
  onApply,
}: {
  entries: Entry[] | null
  onClose: () => void
  onApply: (renames: Rename[]) => Promise<void>
}) {
  const [find, setFind] = useState('')
  const [replace, setReplace] = useState('')
  const [template, setTemplate] = useState('{name}')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!entries) return
    setFind('')
    setReplace('')
    setTemplate('{name}')
    setBusy(false)
    setError('')
  }, [entries])

  const renames = useMemo(
    () =>
      (entries || []).map((entry, index) => ({
        path: entry.path,
        name: expand(template, entry, index, find, replace),
      })),
    [entries, template, find, replace],
  )

  const validation = useMemo(() => {
    const errors = new Map<string, string>()
    const seen = new Set<string>()
    const existing = new Set((entries || []).map((entry) => entry.name.toLowerCase()))
    for (const [index, item] of renames.entries()) {
      const entry = entries![index]
      const name = item.name.trim()
      if (!name) errors.set(item.path, '名称不能为空')
      else if (name === '.' || name === '..' || /[\\/\0]/.test(name))
        errors.set(item.path, '名称包含无效字符')
      else if (seen.has(name.toLowerCase())) errors.set(item.path, '与本次批量名称重复')
      else if (name.toLowerCase() !== entry.name.toLowerCase() && existing.has(name.toLowerCase()))
        errors.set(item.path, '同名项目已存在')
      seen.add(name.toLowerCase())
    }
    return errors
  }, [entries, renames])

  const changed = renames.filter((item, index) => item.name !== entries?.[index]?.name)
  const canApply = !busy && changed.length > 0 && validation.size === 0

  async function submit() {
    if (!canApply) return
    setBusy(true)
    setError('')
    try {
      await onApply(changed)
    } catch (cause) {
      setError(String(cause))
      setBusy(false)
    }
  }

  return (
    <Modal
      open={Boolean(entries)}
      onClose={onClose}
      title={`批量重命名${entries ? ` ${entries.length} 项` : ''}`}
      className="batch-rename-modal"
    >
      <div className="batch-rename">
        <div className="batch-rename-fields">
          <label>
            <span>查找</span>
            <input
              className="text-field"
              value={find}
              placeholder="可选，匹配文件名"
              onChange={(event) => setFind(event.target.value)}
            />
          </label>
          <label>
            <span>替换为</span>
            <input
              className="text-field"
              value={replace}
              placeholder="留空则删除匹配文字"
              onChange={(event) => setReplace(event.target.value)}
            />
          </label>
          <label className="batch-rename-template">
            <span>文件名格式</span>
            <input
              className="text-field"
              value={template}
              spellCheck={false}
              onChange={(event) => setTemplate(event.target.value)}
            />
            <small>
              {'{name}'} 原文件名 · {'{stem}'} 去扩展名 · {'{ext}'} 扩展名 · {'{n}'} / {'{nn}'} /{' '}
              {'{nnn}'} 序号
            </small>
          </label>
        </div>
        <div className="batch-rename-list">
          {(entries || []).map((entry, index) => {
            const item = renames[index]
            const problem = validation.get(entry.path)
            return (
              <div className={`batch-rename-row ${problem ? 'invalid' : ''}`} key={entry.path}>
                <span title={entry.name}>{entry.name}</span>
                <b>→</b>
                <span title={item.name}>{item.name || '（空）'}</span>
                {problem && <em>{problem}</em>}
              </div>
            )
          })}
        </div>
        {error && <p className="batch-rename-error">{error}</p>}
        <footer className="batch-rename-footer">
          <Button variant="outline" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={!canApply}>
            {busy ? '正在重命名…' : `重命名 ${changed.length} 项`}
          </Button>
        </footer>
      </div>
    </Modal>
  )
}

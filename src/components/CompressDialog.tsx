import { useState } from 'react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { archiveFormats, type ArchiveFormat } from '@/lib/archives'

/// Advanced compression: algorithm, level, password and whether the browsing
/// ignore rules apply. The name is still asked for afterwards.
export function CompressDialog({
  format,
  onClose,
  onSubmit,
}: {
  format: ArchiveFormat | null
  onClose: () => void
  onSubmit: (
    format: ArchiveFormat,
    options: { level: number; password: string; full: boolean },
  ) => void
}) {
  const [selected, setSelected] = useState<ArchiveFormat['id']>(format?.id || 'zip')
  const [level, setLevel] = useState(6)
  const [password, setPassword] = useState('')
  const [full, setFull] = useState(false)
  const current = archiveFormats.find((item) => item.id === selected) || archiveFormats[0]
  return (
    <Modal open={Boolean(format)} onClose={onClose} title="高级压缩" className="compress-dialog">
      <form
        className="compress-form"
        onSubmit={(e) => {
          e.preventDefault()
          onSubmit(current, { level, password, full })
        }}
      >
        <label>
          压缩算法
          <select
            className="text-field"
            aria-label="压缩算法"
            value={selected}
            onChange={(e) => setSelected(e.target.value as ArchiveFormat['id'])}
          >
            {archiveFormats.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          压缩等级 <span className="compress-level">{level}</span>
          <input
            type="range"
            aria-label="压缩等级"
            min={1}
            max={9}
            value={level}
            onChange={(e) => setLevel(+e.target.value)}
          />
        </label>
        <label>
          密码
          <input
            className="text-field"
            aria-label="压缩密码"
            type="password"
            value={password}
            disabled={current.id !== 'zip'}
            placeholder={current.id === 'zip' ? '留空则不加密（AES-256）' : '仅 ZIP 支持密码'}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
        <label className="compress-toggle">
          <input
            type="checkbox"
            aria-label="忽略压缩忽略规则"
            checked={full}
            onChange={(e) => setFull(e.target.checked)}
          />
          全量压缩（忽略压缩忽略规则）
        </label>
        <div className="dialog-actions">
          <Button type="button" variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button type="submit">继续</Button>
        </div>
      </form>
    </Modal>
  )
}

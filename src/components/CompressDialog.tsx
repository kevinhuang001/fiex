import { useEffect, useState } from 'react'
import { Archive, LockKeyhole } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { archiveFormats, type ArchiveFormat } from '@/lib/archives'

const algorithms: Record<ArchiveFormat['id'], string> = {
  zip: 'Deflate',
  '7z': 'LZMA2',
  'tar.gz': 'Gzip',
  tar: '仅打包',
  'tar.bz2': 'Bzip2',
  'tar.xz': 'XZ / LZMA2',
}
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
  const [selected, setSelected] = useState<ArchiveFormat['id']>('zip')
  const [level, setLevel] = useState(6)
  const [password, setPassword] = useState('')
  const [full, setFull] = useState(false)
  useEffect(() => {
    if (format) {
      setSelected(format.id)
      setLevel(6)
      setPassword('')
      setFull(false)
    }
  }, [format])
  const current = archiveFormats.find((item) => item.id === selected) || archiveFormats[0]
  const hasLevel = selected !== 'tar' && selected !== '7z'
  return (
    <Modal open={Boolean(format)} onClose={onClose} title="高级压缩" className="compress-dialog">
      <form
        className="archive-form"
        onSubmit={(event) => {
          event.preventDefault()
          onSubmit(current, { level, password: selected === 'zip' ? password : '', full })
        }}
      >
        <div className="archive-formats" role="group" aria-label="压缩格式">
          {archiveFormats.map((item) => (
            <button
              type="button"
              key={item.id}
              aria-pressed={selected === item.id}
              className={`archive-format ${selected === item.id ? 'selected' : ''}`}
              onClick={() => {
                setSelected(item.id)
                if (item.id === 'tar.bz2') setLevel((value) => Math.max(1, value))
              }}
            >
              <Archive size={18} />
              <strong>{item.name}</strong>
              <small>{algorithms[item.id]}</small>
            </button>
          ))}
        </div>
        {hasLevel && (
          <div className="archive-option">
            <label htmlFor="archive-level">
              压缩等级 <span>{level === 0 ? '不压缩' : level}</span>
            </label>
            <input
              id="archive-level"
              type="range"
              aria-label="压缩等级"
              min={selected === 'tar.bz2' ? 1 : 0}
              max={9}
              value={level}
              onChange={(e) => setLevel(+e.target.value)}
            />
            <div className="range-endpoints">
              <span>速度优先</span>
              <span>体积优先</span>
            </div>
          </div>
        )}
        {selected === 'zip' && (
          <div className="archive-option">
            <label htmlFor="archive-password">
              <span>
                <LockKeyhole size={14} /> 加密密码
              </span>
              <small>AES-256</small>
            </label>
            <input
              id="archive-password"
              className="text-field"
              aria-label="压缩密码"
              type="password"
              autoComplete="new-password"
              value={password}
              placeholder="可选"
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>
        )}
        <label className="archive-full">
          <input
            type="checkbox"
            aria-label="忽略压缩忽略规则"
            checked={full}
            onChange={(e) => setFull(e.target.checked)}
          />
          包含忽略的文件
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

import { useEffect, useState } from 'react'
import { Check, Copy, LoaderCircle, FileCheck2 } from 'lucide-react'
import { call } from '@/lib/api'
import { Modal } from './ui/Modal'
interface Hash {
  algorithm: string
  value: string
}
export function ChecksumDialog({ path, onClose }: { path: string | null; onClose: () => void }) {
  const [hashes, setHashes] = useState<Hash[]>([])
  const [error, setError] = useState('')
  const [copied, setCopied] = useState('')
  useEffect(() => {
    if (!path) return
    let active = true
    setHashes([])
    setError('')
    setCopied('')
    void call<Hash[]>('file_checksums', { path })
      .then((result) => {
        if (active) setHashes(result)
      })
      .catch((e) => {
        if (active) setError(String(e))
      })
    return () => {
      active = false
    }
  }, [path])
  return (
    <Modal open={Boolean(path)} onClose={onClose} title="文件校验和" className="checksum-dialog">
      <div className="checksum-file">
        <FileCheck2 size={22} />
        <span title={path || ''}>{path?.split(/[\\/]/).pop()}</span>
      </div>
      {error ? (
        <p role="alert" className="checksum-error">
          {error}
        </p>
      ) : !hashes.length ? (
        <div className="checksum-loading" role="status">
          <LoaderCircle className="spin" size={20} />
          正在计算…
        </div>
      ) : (
        <div className="checksum-list">
          {hashes.map((hash) => (
            <div className="checksum-row" key={hash.algorithm}>
              <div>
                <strong>{hash.algorithm}</strong>
                <code>{hash.value}</code>
              </div>
              <button
                className="icon-button"
                aria-label={`复制 ${hash.algorithm}`}
                title={copied === hash.algorithm ? '已复制' : '复制'}
                onClick={() => {
                  void navigator.clipboard
                    .writeText(hash.value)
                    .then(() => setCopied(hash.algorithm))
                    .catch((e) => setError(String(e)))
                }}
              >
                {copied === hash.algorithm ? <Check size={16} /> : <Copy size={16} />}
              </button>
            </div>
          ))}
        </div>
      )}
      <span className="sr-only" role="status">
        {copied ? `已复制 ${copied}` : ''}
      </span>
    </Modal>
  )
}

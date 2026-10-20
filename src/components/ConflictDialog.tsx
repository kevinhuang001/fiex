import { useEffect, useState } from 'react'
import { ArrowRight, Check, Files, Replace, SkipForward } from 'lucide-react'
import { basename, date, size } from '@/lib/files'
import type { ConflictAction, ConflictResolution, FileVersion, TransferConflict } from '@/lib/types'
import { Button } from './ui/button'
import { Modal } from './ui/Modal'

export function ConflictDialog({
  request,
  busy,
  onClose,
  onResolve,
}: {
  request: { items: TransferConflict[]; moving: boolean } | null
  busy: boolean
  onClose: () => void
  onResolve: (resolutions: ConflictResolution[]) => Promise<void>
}) {
  const [compare, setCompare] = useState(false)
  const [choices, setChoices] = useState<Record<string, { source: boolean; destination: boolean }>>(
    {},
  )
  useEffect(() => {
    setCompare(false)
    setChoices(
      Object.fromEntries(
        (request?.items || []).map((c) => [c.key, { source: true, destination: true }]),
      ),
    )
  }, [request])
  const all = (action: ConflictAction) =>
    void onResolve((request?.items || []).map((c) => ({ key: c.key, action })))
  const version = (file: FileVersion) => (
    <>
      <span title={file.path}>{file.path}</span>
      <small>
        {file.isDir ? '文件夹' : size(file.size)} · {date(file.modified)}
      </small>
    </>
  )
  return (
    <Modal
      open={Boolean(request)}
      onClose={() => {
        if (!busy) onClose()
      }}
      title="替换或跳过文件"
      className="conflict-modal"
    >
      <div className="conflict-summary">
        <Files size={18} />
        <strong>{request?.items.length} 个同名项目</strong>
      </div>
      {!compare ? (
        <div className="conflict-actions">
          <button disabled={busy} onClick={() => all('replace')}>
            <Replace />
            <span>替换目标中的文件</span>
            <ArrowRight />
          </button>
          <button disabled={busy} onClick={() => all('skip')}>
            <SkipForward />
            <span>跳过这些文件</span>
            <ArrowRight />
          </button>
          <button disabled={busy} onClick={() => setCompare(true)}>
            <Files />
            <span>逐个比较文件</span>
            <ArrowRight />
          </button>
        </div>
      ) : (
        <>
          <div className="conflict-comparison" aria-label="比较文件">
            <div className="conflict-head">
              <span>文件</span>
              <span>传入的文件</span>
              <span>目标中的文件</span>
            </div>
            {request?.items.map((c) => (
              <div className="conflict-row" key={c.key}>
                <strong>{basename(c.source.path)}</strong>
                {(['source', 'destination'] as const).map((side) => (
                  <label key={side}>
                    <input
                      type="checkbox"
                      aria-label={`${side === 'source' ? '传入' : '目标'} ${basename(c.source.path)}`}
                      checked={choices[c.key]?.[side] ?? true}
                      onChange={(e) =>
                        setChoices((old) => ({
                          ...old,
                          [c.key]: { ...old[c.key], [side]: e.target.checked },
                        }))
                      }
                    />
                    {version(c[side])}
                  </label>
                ))}
              </div>
            ))}
          </div>
          <div className="dialog-actions">
            <Button variant="ghost" disabled={busy} onClick={() => all('keepBoth')}>
              全部保留两者
            </Button>
            <Button
              disabled={busy}
              onClick={() =>
                void onResolve(
                  (request?.items || []).map((c) => {
                    const choice = choices[c.key]
                    return {
                      key: c.key,
                      action: !choice?.source
                        ? 'skip'
                        : choice.destination
                          ? 'keepBoth'
                          : 'replace',
                    }
                  }),
                )
              }
            >
              <Check size={14} />
              {busy ? '正在处理…' : '继续'}
            </Button>
          </div>
        </>
      )}
    </Modal>
  )
}

import { useEffect, useState } from 'react'
import { Cloud, HardDrive } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { call } from '@/lib/api'
import type { RemoteConnection } from '@/lib/types'

export function ConnectionDialog({
  open,
  onClose,
  onSaved,
}: {
  open: boolean
  onClose: () => void
  onSaved: (connection: RemoteConnection) => void
}) {
  const [kind, setKind] = useState<'webdav' | 'smb'>('webdav')
  const [name, setName] = useState('')
  const [url, setUrl] = useState('')
  const [host, setHost] = useState('')
  const [port, setPort] = useState(445)
  const [share, setShare] = useState('')
  const [base, setBase] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [invalidCerts, setInvalidCerts] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!open) return
    setKind('webdav')
    setName('')
    setUrl('')
    setHost('')
    setPort(445)
    setShare('')
    setBase('')
    setUsername('')
    setPassword('')
    setInvalidCerts(false)
    setError('')
  }, [open])

  const save = async (event: React.FormEvent) => {
    event.preventDefault()
    setBusy(true)
    setError('')
    try {
      const connection = await call<RemoteConnection>('save_remote_connection', {
        input: {
          name,
          kind,
          url,
          host,
          port,
          share,
          base,
          username,
          password,
          allowInvalidCerts: invalidCerts,
        },
      })
      onSaved(connection)
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal open={open} onClose={onClose} title="添加连接">
      <form className="connection-form" onSubmit={save}>
        <div className="connection-kinds">
          <button
            type="button"
            className={kind === 'webdav' ? 'active' : ''}
            onClick={() => setKind('webdav')}
          >
            <Cloud size={15} />
            WebDAV
          </button>
          <button
            type="button"
            className={kind === 'smb' ? 'active' : ''}
            onClick={() => setKind('smb')}
          >
            <HardDrive size={15} />
            SMB
          </button>
        </div>
        <label>
          <span>名称</span>
          <input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
        </label>
        {kind === 'webdav' ? (
          <>
            <label>
              <span>地址</span>
              <input
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://example.com/dav"
              />
            </label>
            <label className="connection-check">
              <input
                type="checkbox"
                checked={invalidCerts}
                onChange={(e) => setInvalidCerts(e.target.checked)}
              />
              允许自签名证书
            </label>
          </>
        ) : (
          <div className="connection-grid">
            <label>
              <span>主机</span>
              <input
                value={host}
                onChange={(e) => setHost(e.target.value)}
                placeholder="nas.local"
              />
            </label>
            <label>
              <span>端口</span>
              <input
                type="number"
                min="1"
                max="65535"
                aria-label="SMB 端口"
                value={port}
                onChange={(e) => setPort(+e.target.value)}
              />
            </label>
            <label>
              <span>共享名</span>
              <input
                value={share}
                onChange={(e) => setShare(e.target.value)}
                placeholder="Public"
              />
            </label>
            <label className="connection-wide">
              <span>子路径</span>
              <input value={base} onChange={(e) => setBase(e.target.value)} placeholder="可选" />
            </label>
          </div>
        )}
        <div className="connection-grid">
          <label>
            <span>用户名</span>
            <input value={username} onChange={(e) => setUsername(e.target.value)} />
          </label>
          <label>
            <span>密码</span>
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
          </label>
        </div>
        {error && <p className="connection-error">{error}</p>}
        <footer>
          <Button type="button" onClick={onClose}>
            取消
          </Button>
          <Button type="submit" disabled={busy || !name.trim()}>
            {busy ? '连接中…' : '添加'}
          </Button>
        </footer>
      </form>
    </Modal>
  )
}

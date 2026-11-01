import { useEffect, useState } from 'react'
import { t } from '@/i18n'
import { call, native } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { SelectField } from '../ui/SelectField'
import { KeyRound } from 'lucide-react'

export function CredentialSettings() {
  const [backend, setBackend] = useState('local')
  const [busy, setBusy] = useState(true)
  useEffect(() => {
    if (!native) {
      setBusy(false)
      return
    }
    let alive = true
    void call<string>('credential_backend')
      .then((value) => {
        if (alive) {
          setBackend(value)
          setBusy(false)
        }
      })
      .catch((cause) => {
        if (alive) {
          reportError(cause, 'credentials')
          setBusy(false)
        }
      })
    return () => {
      alive = false
    }
  }, [])
  const change = async (next: string) => {
    if (busy || next === backend) return
    setBusy(true)
    try {
      if (native) await call('credential_backend_set', { backend: next })
      setBackend(next)
    } catch (cause) {
      reportError(cause, 'credentials')
    } finally {
      setBusy(false)
    }
  }
  return (
    <>
      <div className="settings-row" aria-busy={busy}>
        <div className="settings-row-label">
          <span className="settings-row-icon" style={{ background: '#f59e0b' }}>
            <KeyRound size={15} />
          </span>
          {t('凭据存储')}
        </div>
        <div className="settings-row-control">
          <fieldset disabled={busy} style={{ border: 0, padding: 0, margin: 0 }}>
            <SelectField
              label={t('凭据存储')}
              value={backend}
              options={[
                { value: 'local', label: t('本地文件') },
                { value: 'oskeyring', label: t('系统钥匙串') },
              ]}
              onChange={(next) => void change(next)}
            />
          </fieldset>
        </div>
      </div>
    </>
  )
}

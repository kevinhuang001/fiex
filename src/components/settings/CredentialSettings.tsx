import { useEffect, useState } from 'react'
import { t } from '@/i18n'
import { call, native } from '@/lib/api'
import { SelectField } from '../ui/SelectField'

export function CredentialSettings() {
  const [backend, setBackend] = useState('local')
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState('')
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
          setError(String(cause))
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
    setError('')
    try {
      if (native) await call('credential_backend_set', { backend: next })
      setBackend(next)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }
  return (
    <>
      <div className="settings-row" aria-busy={busy}>
        <div className="settings-row-label">{t('凭据存储')}</div>
        <div className="settings-row-control">
          <fieldset disabled={busy} style={{ border: 0, padding: 0, margin: 0 }}>
            <SelectField
              label={t('凭据存储')}
              value={backend}
              options={[
                { value: 'local', label: t('本地文件（默认）') },
                { value: 'oskeyring', label: t('系统钥匙串') },
              ]}
              onChange={(next) => void change(next)}
            />
          </fieldset>
        </div>
      </div>
      {error && <p role="alert">{error}</p>}
    </>
  )
}

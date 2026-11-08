import { fixtureGpgKeys } from './data'

/** Browser-demo key creation/deletion. Native builds execute the user's gpg. */
export function createFixtureGpgKey(params: Record<string, unknown>) {
  const fingerprint = crypto.randomUUID().replaceAll('-', '').toUpperCase().slice(0, 40)
  const name = String(params.name || '').trim()
  const email = String(params.email || '').trim()
  fixtureGpgKeys.push({
    fingerprint,
    keyId: fingerprint.slice(-8),
    name: `${name} <${email}>`,
    email,
    expires: '',
    hasSecret: true,
  })
  return { ok: true, fingerprint }
}

export function deleteFixtureGpgKey(params: Record<string, unknown>) {
  const fingerprint = String(params.fingerprint || '')
  const index = fixtureGpgKeys.findIndex((key) => key.fingerprint === fingerprint)
  if (index >= 0) fixtureGpgKeys.splice(index, 1)
  return { ok: true }
}

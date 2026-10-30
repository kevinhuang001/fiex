import { describe, expect, it } from 'vitest'
import { redactDiagnostic } from './redactDiagnostic'

describe('diagnostic redaction', () => {
  it('scrubs multiline headers, JSON and OAuth query strings before UI/export', () => {
    for (const value of [
      'Authorization: Bearer HIDDEN\n continuationHIDDEN',
      'Cookie: session=HIDDEN; auth=HIDDEN',
      'Set-Cookie: session=HIDDEN',
      'X-Api-Key: HIDDEN',
      '{"password": "HIDDEN with spaces", "refresh_token":\n"HIDDEN"}',
      'https://host/?code=HIDDEN&token=HIDDEN&key=HIDDEN',
    ])
      expect(redactDiagnostic(value)).not.toContain('HIDDEN')
  })
})

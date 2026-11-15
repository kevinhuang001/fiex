import { describe, expect, it } from 'vitest'
import type { InfoFieldDelta, InfoSection } from '@/lib/ipcValidation/infoTypes'
import { appendPendingInfoDelta, mergeInfoFieldUpdates } from './model'

const section: InfoSection = {
  id: 'core.media',
  title: 'Media',
  providerId: 'core.media',
  provider: 'Fiex',
  source: 'core',
  fields: [
    {
      id: 'duration',
      subsectionId: 'audio',
      label: 'Duration',
      compute: 'onDemand',
      status: 'idle',
    },
  ],
}

function delta(revision: number, value: string): InfoFieldDelta {
  return {
    sessionId: 'session-1',
    revision,
    updates: [
      {
        providerId: 'core.media',
        subsectionId: 'audio',
        fieldId: 'duration',
        status: 'ready',
        value,
      },
    ],
  }
}

describe('mergeInfoFieldUpdates', () => {
  it('applies updates by provider, subsection, and field without mutating the input', () => {
    const result = mergeInfoFieldUpdates([section], delta(1, '02:18').updates)

    expect(result[0]?.fields[0]).toMatchObject({ status: 'ready', value: '02:18' })
    expect(section.fields[0]).toMatchObject({ status: 'idle' })
  })

  it('returns the original sections when the delta is empty', () => {
    const sections = [section]
    expect(mergeInfoFieldUpdates(sections, [])).toBe(sections)
  })
})

describe('appendPendingInfoDelta', () => {
  it('coalesces early updates by field and ignores stale revisions', () => {
    const first = appendPendingInfoDelta(undefined, delta(2, 'old'))
    const latest = appendPendingInfoDelta(first ?? undefined, delta(3, 'new'))
    const stale = appendPendingInfoDelta(latest ?? undefined, delta(2, 'stale'))

    expect(latest?.revision).toBe(3)
    expect([...(latest?.updates.values() ?? [])]).toEqual(delta(3, 'new').updates)
    expect(stale).toBeNull()
  })
})

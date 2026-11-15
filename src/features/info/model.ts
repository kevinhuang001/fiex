import type {
  InfoField,
  InfoFieldDelta,
  InfoFieldUpdate,
  InfoSection,
} from '@/lib/ipcValidation/infoTypes'

export type PendingInfoUpdates = {
  revision: number
  updates: Map<string, InfoFieldUpdate>
}

function updateKey(update: InfoFieldUpdate) {
  return `${update.providerId}\u0000${update.subsectionId}\u0000${update.fieldId}`
}

export function appendPendingInfoDelta(
  previous: PendingInfoUpdates | undefined,
  delta: InfoFieldDelta,
): PendingInfoUpdates | null {
  if (previous && delta.revision <= previous.revision) return null
  const updates = new Map(previous?.updates)
  for (const update of delta.updates) updates.set(updateKey(update), update)
  return { revision: delta.revision, updates }
}

export function mergeInfoFieldUpdates(
  sections: InfoSection[],
  updates: InfoFieldUpdate[],
): InfoSection[] {
  if (!updates.length) return sections
  const byKey = new Map(updates.map((update) => [updateKey(update), update]))
  return sections.map((section) => ({
    ...section,
    fields: section.fields.map((field) => {
      const update = byKey.get(`${section.providerId}\u0000${field.subsectionId}\u0000${field.id}`)
      if (!update) return field
      const next: InfoField = { ...field, status: update.status }
      if (update.value === undefined) delete next.value
      else next.value = update.value
      if (update.error === undefined) delete next.error
      else next.error = update.error
      return next
    }),
  }))
}

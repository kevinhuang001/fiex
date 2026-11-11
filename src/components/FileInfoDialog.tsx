import { t } from '@/i18n'
import { useEffect, useRef, useState } from 'react'
import { Modal } from './ui/Modal'
import { FileIcon } from './FileIcon'
import { FileInfoSections } from './FileInfoSections'
import { tagHex } from '@/lib/tags'
import { call, native } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { parseInfoFieldDelta, shouldApplyInfoDelta } from '@/lib/ipcValidation'
import type { Entry } from '@/lib/types'
import { Channel } from '@tauri-apps/api/core'

export type InfoField = {
  id: string
  subsectionId: string
  label: string
  compute: 'onOpen' | 'onDemand'
  status: 'idle' | 'pending' | 'ready' | 'unavailable' | 'error'
  value?: string
  error?: string
}
export type InfoSection = {
  id: string
  title: string
  providerId: string
  provider: string
  source: 'core' | 'plugin'
  pluginId?: string
  fields: InfoField[]
  error?: string
}
type InfoFieldUpdate = {
  providerId: string
  subsectionId: string
  fieldId: string
  status: InfoField['status']
  value?: string
  error?: string
}
export type InfoFieldDelta = { sessionId: string; revision: number; updates: InfoFieldUpdate[] }
export type InfoOpenResult = { sessionId: string; sections: InfoSection[] }

function mergeFieldUpdates(sections: InfoSection[], updates: InfoFieldUpdate[]) {
  if (!updates.length) return sections
  const byKey = new Map(
    updates.map((update) => [
      `${update.providerId}\u0000${update.subsectionId}\u0000${update.fieldId}`,
      update,
    ]),
  )
  return sections.map((section) => ({
    ...section,
    fields: section.fields.map((field) => {
      const update = byKey.get(`${section.providerId}\u0000${field.subsectionId}\u0000${field.id}`)
      return update
        ? { ...field, status: update.status, value: update.value, error: update.error }
        : field
    }),
  }))
}

export function FileInfoDialog({ info, onClose }: { info: Entry | null; onClose: () => void }) {
  const [sections, setSections] = useState<InfoSection[]>([])
  const [loading, setLoading] = useState(false)
  const [copied, setCopied] = useState('')
  const copiedTimer = useRef<number | null>(null)
  const sessionId = useRef<string | null>(null)
  const revision = useRef(0)
  const tasks = useRef(new Set<string>())
  const earlyUpdates = useRef(
    new Map<string, { revision: number; updates: Map<string, InfoFieldUpdate> }>(),
  )

  useEffect(
    () => () => {
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current)
    },
    [],
  )

  useEffect(() => {
    if (!info) return
    let alive = true
    let activeSession: string | null = null
    revision.current = 0
    setSections([])
    setLoading(true)
    earlyUpdates.current.clear()
    const onUpdate = native ? new Channel<unknown>() : undefined
    if (onUpdate)
      onUpdate.onmessage = (raw) => {
        if (!alive) return
        let delta: InfoFieldDelta
        try {
          delta = parseInfoFieldDelta(raw)
        } catch (cause) {
          reportError(cause, 'file-info:ipc')
          return
        }
        if (activeSession === null) {
          const queued = earlyUpdates.current.get(delta.sessionId)
          if (queued && delta.revision <= queued.revision) return
          const updates = new Map(queued?.updates)
          for (const update of delta.updates) {
            updates.set(
              `${update.providerId}\u0000${update.subsectionId}\u0000${update.fieldId}`,
              update,
            )
          }
          earlyUpdates.current.set(delta.sessionId, { revision: delta.revision, updates })
          return
        }
        if (!shouldApplyInfoDelta(delta, activeSession, revision.current)) return
        revision.current = delta.revision
        setSections((previous) => mergeFieldUpdates(previous, delta.updates))
      }
    const open = async () => {
      try {
        const result = await call('file_info_open', {
          path: info.path,
          ...(onUpdate ? { onUpdate } : {}),
        })
        activeSession = result.sessionId
        if (!alive) {
          await call('file_info_session_close', { sessionId: activeSession }).catch(() => {})
          return
        }
        sessionId.current = activeSession
        const queued = earlyUpdates.current.get(activeSession)
        earlyUpdates.current.clear()
        if (alive && Array.isArray(result.sections)) {
          if (queued) revision.current = queued.revision
          setSections(
            mergeFieldUpdates(result.sections, queued ? [...queued.updates.values()] : []),
          )
        }
      } catch (cause) {
        if (alive) reportError(cause, 'file-info')
      } finally {
        if (alive) setLoading(false)
      }
    }
    void open()
    return () => {
      alive = false
      if (sessionId.current === activeSession) sessionId.current = null
      for (const taskId of tasks.current) {
        if (activeSession)
          void call('file_info_cancel', { sessionId: activeSession, taskId }).catch(() => {})
      }
      tasks.current.clear()
      if (activeSession)
        void call('file_info_session_close', { sessionId: activeSession }).catch(() => {})
    }
  }, [info?.path, info?.name, info?.isDir, info?.isSymlink, info?.size, info?.modified, info?.mime])

  const compute = async (section: InfoSection, field: InfoField) => {
    const activeSession = sessionId.current
    if (!activeSession || tasks.current.size > 3) return
    const taskId = crypto.randomUUID()
    tasks.current.add(taskId)
    setSections((previous) =>
      previous.map((item) =>
        item.id !== section.id
          ? item
          : {
              ...item,
              fields: item.fields.map((candidate) =>
                candidate.id === field.id && candidate.subsectionId === field.subsectionId
                  ? { ...candidate, status: 'pending', error: undefined }
                  : candidate,
              ),
            },
      ),
    )
    try {
      const result = parseInfoFieldDelta(
        await call('file_info_compute', {
          sessionId: activeSession,
          providerId: section.providerId,
          fields: [{ subsectionId: field.subsectionId, fieldId: field.id }],
          taskId,
        }),
      )
      if (
        sessionId.current === activeSession &&
        shouldApplyInfoDelta(result, activeSession, revision.current)
      ) {
        revision.current = result.revision
        setSections((previous) => mergeFieldUpdates(previous, result.updates))
      }
    } catch (cause) {
      if (sessionId.current === activeSession) {
        setSections((previous) =>
          previous.map((item) =>
            item.id !== section.id
              ? item
              : {
                  ...item,
                  fields: item.fields.map((candidate) =>
                    candidate.id === field.id && candidate.subsectionId === field.subsectionId
                      ? { ...candidate, status: 'error', error: String(cause) }
                      : candidate,
                  ),
                },
          ),
        )
      }
    } finally {
      tasks.current.delete(taskId)
    }
  }

  const copy = async (value: string, key: string) => {
    try {
      await navigator.clipboard.writeText(value)
      setCopied(key)
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current)
      copiedTimer.current = window.setTimeout(() => setCopied(''), 1200)
    } catch {
      // Keep the value selectable when clipboard access is unavailable.
    }
  }

  return (
    <Modal open={!!info} onClose={onClose} title={t('显示简介')} className="file-info-modal">
      {info && (
        <div className="file-info">
          <FileIcon entry={info} size={72} />
          <h3>{info.name}</h3>
          <div className="file-info-scroll">
            <dl>
              <FileInfoSections
                sections={sections}
                copied={copied}
                computeDisabled={tasks.current.size > 3}
                onCopy={(value, key) => void copy(value, key)}
                onCompute={(section, field) => void compute(section, field)}
              />
            </dl>
            {loading && <p className="file-info-status">{t('正在读取信息…')}</p>}
            {info.tags.length > 0 && (
              <div className="file-info-tags">
                {info.tags.map((tag) => (
                  <span className="tag-chip" key={`${tag.name}:${tag.color}`}>
                    <i style={{ background: tagHex(tag) }} />
                    {tag.name}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </Modal>
  )
}

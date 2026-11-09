import { t } from '@/i18n'
import { Check, Copy } from 'lucide-react'
import { Fragment, useEffect, useRef, useState } from 'react'
import { Modal } from './ui/Modal'
import { FileIcon } from './FileIcon'
import { tagHex } from '@/lib/tags'
import { call, native } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { parseInfoFieldDelta } from '@/lib/ipcValidation'
import type { Entry } from '@/lib/types'
import { Channel } from '@tauri-apps/api/core'

type InfoField = {
  id: string
  sectionId: string
  label: string
  compute: 'onOpen' | 'onDemand'
  status: 'idle' | 'pending' | 'ready' | 'unavailable' | 'error'
  value?: string
  error?: string
}
type InfoSection = {
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
  sectionId: string
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
      `${update.providerId}\u0000${update.sectionId}\u0000${update.fieldId}`,
      update,
    ]),
  )
  return sections.map((section) => ({
    ...section,
    fields: section.fields.map((field) => {
      const update = byKey.get(`${section.providerId}\u0000${field.sectionId}\u0000${field.id}`)
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
  const tasks = useRef(new Set<string>())
  const earlyUpdates = useRef(new Map<string, InfoFieldUpdate>())

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
        for (const update of delta.updates) {
          earlyUpdates.current.set(
            `${update.providerId}\u0000${update.sectionId}\u0000${update.fieldId}`,
            update,
          )
        }
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
        if (alive && Array.isArray(result.sections)) {
          setSections(mergeFieldUpdates(result.sections, [...earlyUpdates.current.values()]))
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
                candidate.id === field.id && candidate.sectionId === field.sectionId
                  ? { ...candidate, status: 'pending', error: undefined }
                  : candidate,
              ),
            },
      ),
    )
    try {
      const result = await call('file_info_compute', {
        sessionId: activeSession,
        providerId: section.providerId,
        fields: [{ sectionId: field.sectionId, fieldId: field.id }],
        taskId,
      })
      if (sessionId.current === activeSession && result.sessionId === activeSession)
        setSections((previous) => mergeFieldUpdates(previous, result.updates))
    } catch (cause) {
      if (sessionId.current === activeSession) {
        setSections((previous) =>
          previous.map((item) =>
            item.id !== section.id
              ? item
              : {
                  ...item,
                  fields: item.fields.map((candidate) =>
                    candidate.id === field.id && candidate.sectionId === field.sectionId
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
              {sections.map((section, sectionIndex) => (
                <Fragment key={section.id}>
                  <div className="file-info-group">
                    <span>{section.title}</span>
                    {section.source === 'plugin' && (
                      <small>{t('来自：{0}', section.provider)}</small>
                    )}
                  </div>
                  {section.error && (
                    <div className="file-info-row">
                      <dt>{t('提供程序')}</dt>
                      <dd>{section.error}</dd>
                    </div>
                  )}
                  {section.fields.map((field, fieldIndex) => {
                    const key = `${sectionIndex}:${fieldIndex}`
                    const value =
                      field.status === 'ready'
                        ? field.value
                        : field.status === 'unavailable'
                          ? t('不可用')
                          : field.status === 'error'
                            ? field.error
                            : field.compute === 'onDemand'
                              ? t('尚未计算')
                              : t('正在计算…')
                    return (
                      <div className="file-info-row" key={`${field.sectionId}:${field.id}`}>
                        <dt>{field.label}</dt>
                        <dd>
                          <span>{value}</span>
                          {field.status === 'ready' && field.value && (
                            <button
                              type="button"
                              className="info-copy"
                              title={copied === key ? t('已复制') : t('复制')}
                              aria-label={t('复制')}
                              onClick={() => void copy(field.value!, key)}
                            >
                              {copied === key ? <Check size={12} /> : <Copy size={12} />}
                            </button>
                          )}
                          {field.compute === 'onDemand' &&
                            ['idle', 'error'].includes(field.status) && (
                              <button
                                type="button"
                                className="file-info-action"
                                disabled={tasks.current.size > 3}
                                onClick={() => void compute(section, field)}
                              >
                                {field.status === 'error' ? t('重试并授权') : t('计算')}
                              </button>
                            )}
                          {field.compute === 'onDemand' && field.status === 'pending' && (
                            <button type="button" className="file-info-action" disabled>
                              {t('正在计算…')}
                            </button>
                          )}
                        </dd>
                      </div>
                    )
                  })}
                </Fragment>
              ))}
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

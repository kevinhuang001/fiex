import type { InfoField, InfoFieldDelta, InfoSection } from '@/lib/ipcValidation/infoTypes'
import { parseInfoFieldDelta, shouldApplyInfoDelta } from '@/lib/ipcValidation/info'
import type { Entry } from '@/lib/types'
import { useCallback, useEffect, useRef, useState } from 'react'
import {
  appendPendingInfoDelta,
  markInfoFieldPending,
  mergeInfoFieldUpdates,
  type PendingInfoUpdates,
} from '../model'
import type { InfoSessionServices, InfoSessionState } from '../types'

export function useInfoSession(
  info: Entry | null,
  services: InfoSessionServices,
): InfoSessionState {
  const [sections, setSections] = useState<InfoSection[]>([])
  const [loading, setLoading] = useState(false)
  const [taskCount, setTaskCount] = useState(0)
  const sessionId = useRef<string | null>(null)
  const revision = useRef(0)
  const tasks = useRef(new Set<string>())
  const earlyUpdates = useRef(new Map<string, PendingInfoUpdates>())
  const infoPath = info?.path
  const infoIsDir = info?.isDir
  const infoIsSymlink = info?.isSymlink
  const infoMime = info?.mime
  const infoModified = info?.modified
  const infoName = info?.name
  const infoSize = info?.size

  useEffect(() => {
    if (infoPath === undefined) {
      setSections([])
      setLoading(false)
      setTaskCount(0)
      return
    }

    let alive = true
    let activeSession: string | null = null
    revision.current = 0
    setSections([])
    setLoading(true)
    earlyUpdates.current.clear()
    const activeTasks = tasks.current

    const onUpdate = (raw: unknown) => {
      if (!alive) return
      let delta: InfoFieldDelta
      try {
        delta = parseInfoFieldDelta(raw)
      } catch (cause) {
        services.reportError(cause, 'file-info:ipc')
        return
      }

      if (activeSession === null) {
        const next = appendPendingInfoDelta(earlyUpdates.current.get(delta.sessionId), delta)
        if (next) earlyUpdates.current.set(delta.sessionId, next)
        return
      }
      if (!shouldApplyInfoDelta(delta, activeSession, revision.current)) return
      revision.current = delta.revision
      setSections((previous) => mergeInfoFieldUpdates(previous, delta.updates))
    }

    const open = async () => {
      try {
        const result = await services.open(infoPath, onUpdate)
        activeSession = result.sessionId
        if (!alive) {
          await services.close(activeSession).catch(() => {})
          return
        }
        sessionId.current = activeSession
        const queued = earlyUpdates.current.get(activeSession)
        earlyUpdates.current.clear()
        if (Array.isArray(result.sections)) {
          if (queued) revision.current = queued.revision
          setSections(
            mergeInfoFieldUpdates(result.sections, queued ? [...queued.updates.values()] : []),
          )
        }
      } catch (cause) {
        if (alive) services.reportError(cause, 'file-info')
      } finally {
        if (alive) setLoading(false)
      }
    }

    void open()
    return () => {
      alive = false
      if (sessionId.current === activeSession) sessionId.current = null
      for (const taskId of activeTasks) {
        if (activeSession) void services.cancel(activeSession, taskId).catch(() => {})
      }
      activeTasks.clear()
      setTaskCount(0)
      if (activeSession) void services.close(activeSession).catch(() => {})
    }
  }, [infoIsDir, infoIsSymlink, infoMime, infoModified, infoName, infoPath, infoSize, services])

  const compute = useCallback(
    async (section: InfoSection, field: InfoField) => {
      const activeSession = sessionId.current
      if (!activeSession || tasks.current.size > 3) return
      const taskId = services.createTaskId()
      tasks.current.add(taskId)
      setTaskCount(tasks.current.size)
      setSections((previous) =>
        previous.map((item) =>
          item.id !== section.id
            ? item
            : {
                ...item,
                fields: item.fields.map((candidate) =>
                  candidate.id === field.id && candidate.subsectionId === field.subsectionId
                    ? markInfoFieldPending(candidate)
                    : candidate,
                ),
              },
        ),
      )
      try {
        const result = parseInfoFieldDelta(
          await services.compute({
            sessionId: activeSession,
            providerId: section.providerId,
            subsectionId: field.subsectionId,
            fieldId: field.id,
            taskId,
          }),
        )
        if (
          sessionId.current === activeSession &&
          shouldApplyInfoDelta(result, activeSession, revision.current)
        ) {
          revision.current = result.revision
          setSections((previous) => mergeInfoFieldUpdates(previous, result.updates))
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
        setTaskCount(tasks.current.size)
      }
    },
    [services],
  )

  return { sections, loading, computeDisabled: taskCount > 3, compute }
}

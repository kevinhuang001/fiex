import { t } from '@/i18n'
import { Channel } from '@tauri-apps/api/core'
import { useSyncExternalStore } from 'react'
import { demo, native } from '@/lib/api'
import type { OperationProgress } from '@/lib/types'

type ActivityStatus = 'running' | 'done' | 'error'
export type ActivityKind = 'transfer' | 'delete' | 'sync' | 'plugin'

export interface ActivityItem {
  id: string
  kind: ActivityKind
  label: string
  status: ActivityStatus
  startedAt: number
  finishedAt?: number
  detail?: string
  progress?: number
  indeterminate?: boolean
  done?: number
  total?: number
  step?: string
  /** `bytes` renders sizes instead of file counts in the details pane. */
  unit?: 'bytes' | 'files'
  /** Plugin identity for per-plugin filtering in the status centre. */
  pluginId?: string
}

const LIMIT = 60
const KEY = 'fiex:activity'

let items: ActivityItem[] = load()
let open = false
let seq = 0
const listeners = new Set<() => void>()
const cancellers = new Map<string, () => void>()

function load(): ActivityItem[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) || '[]')
    if (!Array.isArray(parsed)) return []
    // Anything that was running when the window closed is history now.
    return parsed
      .filter(
        (item) =>
          item &&
          typeof item.label === 'string' &&
          (item.kind === 'transfer' ||
            item.kind === 'delete' ||
            item.kind === 'sync' ||
            item.kind === 'plugin'),
      )
      .slice(0, LIMIT)
      .map((item): ActivityItem => ({
        id: String(item.id ?? Math.random()),
        kind: item.kind as ActivityKind,
        label: String(item.label),
        status: item.status === 'running' ? 'error' : (item.status as ActivityStatus),
        startedAt: Number(item.startedAt) || Date.now(),
        finishedAt: item.finishedAt ? Number(item.finishedAt) : undefined,
        detail: item.status === 'running' ? t('应用已关闭') : item.detail,
        indeterminate: Boolean(item.indeterminate),
        pluginId:
          item.kind === 'plugin' && typeof item.pluginId === 'string' ? item.pluginId : undefined,
      }))
  } catch {
    return []
  }
}

function persist() {
  try {
    localStorage.setItem(KEY, JSON.stringify(items.slice(0, LIMIT)))
  } catch {
    // Storage is best-effort.
  }
}

function emit() {
  listeners.forEach((listener) => listener())
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function beginActivity(
  rawLabel: string,
  detail?: string,
  cancel?: () => void,
  kind: ActivityKind = 'transfer',
  pluginId?: string,
  reveal = true,
): string {
  const label = rawLabel.replace(/^正在/, '').replace(/…$/, '')
  const id = `${Date.now()}-${++seq}`
  const source = kind === 'plugin' && pluginId ? { pluginId } : {}
  items = [
    {
      id,
      kind,
      label,
      detail,
      status: 'running' as const,
      startedAt: Date.now(),
      ...source,
    },
    ...items,
  ].slice(0, LIMIT)
  // Transfers and syncs surface their progress; other operations stay in
  // history without covering the window. A quiet activity is recorded for
  // history but does not open the panel.
  if (reveal && (kind === 'transfer' || kind === 'delete' || kind === 'sync' || kind === 'plugin'))
    open = true
  if (cancel) cancellers.set(id, cancel)
  persist()
  emit()
  return id
}

export function updateActivity(
  id: string,
  patch: Partial<
    Pick<
      ActivityItem,
      'label' | 'progress' | 'indeterminate' | 'done' | 'total' | 'step' | 'detail' | 'unit'
    >
  >,
) {
  let changed = false
  items = items.map((item) => {
    if (item.id !== id || item.status !== 'running') return item
    changed = true
    return {
      ...item,
      ...patch,
      progress:
        patch.progress === undefined ? item.progress : Math.max(0, Math.min(1, patch.progress)),
    }
  })
  // Progress arrives once per file; keep it in memory and persist only the
  // lifecycle changes to avoid a localStorage write on every tick.
  if (changed) emit()
}

/** A progress channel for actions that have no activity row to update. */
export function quietProgressChannel() {
  if (!native) return undefined
  return new Channel<OperationProgress>()
}

export function progressChannel(id: string) {
  // The browser fixture has no Tauri internals, so it receives a plain object
  // whose `onmessage` the demo mock can call to simulate a progress channel.
  if (!native) {
    if (!demo) return undefined
    const channel = {
      onmessage: undefined as ((progress: OperationProgress) => void) | undefined,
    }
    channel.onmessage = ({ done, total, step, unit }) =>
      updateActivity(id, {
        done,
        total,
        progress: total > 0 ? done / total : undefined,
        step: step || undefined,
        unit,
      })
    return channel as unknown as Channel<OperationProgress>
  }
  const channel = new Channel<OperationProgress>()
  channel.onmessage = ({ done, total, step, unit }) =>
    updateActivity(id, {
      done,
      total,
      progress: total > 0 ? done / total : undefined,
      step: step || undefined,
      unit,
    })
  return channel
}

export function cancelActivity(id: string) {
  cancellers.get(id)?.()
  cancellers.delete(id)
  items = items.map((item) =>
    item.id === id
      ? { ...item, status: 'error', finishedAt: Date.now(), detail: t('已取消') }
      : item,
  )
  persist()
  emit()
}

export function isCancellable(id: string) {
  return cancellers.has(id)
}

export function finishActivity(id: string, error?: string, detail?: string) {
  cancellers.delete(id)
  items = items.map((item) =>
    item.id === id
      ? item.status === 'error' && item.detail === t('已取消')
        ? item
        : {
            ...item,
            status: error ? 'error' : 'done',
            finishedAt: Date.now(),
            detail: error || detail || item.detail,
          }
      : item,
  )
  persist()
  emit()
}

export function listActivity(): ActivityItem[] {
  return items
}

/** Adds a finished entry without opening the panel: background cache and pin
 *  transitions belong in history without interrupting the user. */
export function recordActivity(
  label: string,
  detail?: string,
  kind: ActivityKind = 'sync',
  status: 'done' | 'error' = 'done',
) {
  const id = `${Date.now()}-${++seq}`
  items = [
    {
      id,
      kind,
      label,
      detail,
      status,
      startedAt: Date.now(),
      finishedAt: Date.now(),
    },
    ...items,
  ].slice(0, LIMIT)
  persist()
  emit()
}

function clearActivity() {
  items = items.filter((item) => item.status === 'running')
  persist()
  emit()
}

function setOpen(next: boolean) {
  open = next
  emit()
}

export function useActivity() {
  const snapshot = useSyncExternalStore(subscribe, () => items)
  const isOpen = useSyncExternalStore(subscribe, () => open)
  return { items: snapshot, open: isOpen, setOpen, clear: clearActivity }
}

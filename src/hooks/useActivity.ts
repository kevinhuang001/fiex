import { Channel } from '@tauri-apps/api/core'
import { useSyncExternalStore } from 'react'
import { native } from '@/lib/api'
import type { OperationProgress } from '@/lib/types'

export type ActivityStatus = 'running' | 'done' | 'error'
export type ActivityKind = 'transfer' | 'git' | 'delete' | 'sync' | 'plugin'

export interface ActivityItem {
  id: string
  kind: ActivityKind
  label: string
  status: ActivityStatus
  startedAt: number
  finishedAt?: number
  detail?: string
  progress?: number
  done?: number
  total?: number
  step?: string
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
            item.kind === 'git' ||
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
        detail: item.status === 'running' ? '应用已关闭' : item.detail,
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
): string {
  const label = rawLabel.replace(/^正在/, '').replace(/…$/, '')
  const id = `${Date.now()}-${++seq}`
  items = [
    { id, kind, label, detail, status: 'running' as const, startedAt: Date.now() },
    ...items,
  ].slice(0, LIMIT)
  // Transfers and syncs surface their progress; other operations stay in
  // history without covering the window.
  if (kind === 'transfer' || kind === 'sync') open = true
  if (cancel) cancellers.set(id, cancel)
  persist()
  emit()
  return id
}

export function updateActivity(
  id: string,
  patch: Partial<Pick<ActivityItem, 'label' | 'progress' | 'done' | 'total' | 'step' | 'detail'>>,
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

export function progressChannel(id: string) {
  // The browser fixture has no Tauri internals; channels only exist in the
  // desktop webview.
  if (!native) return undefined
  const channel = new Channel<OperationProgress>()
  channel.onmessage = ({ done, total, step }) =>
    updateActivity(id, {
      done,
      total,
      progress: total > 0 ? done / total : undefined,
      step: step || undefined,
    })
  return channel
}

export function cancelActivity(id: string) {
  cancellers.get(id)?.()
  cancellers.delete(id)
  items = items.map((item) =>
    item.id === id ? { ...item, status: 'error', finishedAt: Date.now(), detail: '已取消' } : item,
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
      ? item.status === 'error' && item.detail === '已取消'
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

export function clearActivity() {
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

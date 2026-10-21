import { useSyncExternalStore } from 'react'

export type ActivityStatus = 'running' | 'done' | 'error'

export interface ActivityItem {
  id: string
  label: string
  status: ActivityStatus
  startedAt: number
  finishedAt?: number
  detail?: string
  progress?: number
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
      .filter((item) => item && typeof item.label === 'string')
      .slice(0, LIMIT)
      .map((item): ActivityItem => ({
        id: String(item.id ?? Math.random()),
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

export function beginActivity(rawLabel: string, detail?: string, cancel?: () => void): string {
  const label = rawLabel.replace(/^正在/, '').replace(/…$/, '')
  const id = `${Date.now()}-${++seq}`
  items = [
    { id, label, detail, status: 'running' as const, startedAt: Date.now() },
    ...items,
  ].slice(0, LIMIT)
  // Transfers and archive work show their progress by default; quick commands
  // stay in the history without covering the window.
  if (/复制|移动|压缩|解压/.test(label)) open = true
  if (cancel) cancellers.set(id, cancel)
  persist()
  emit()
  return id
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
      ? {
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

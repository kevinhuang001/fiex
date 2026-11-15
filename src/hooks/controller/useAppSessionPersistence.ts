import { useEffect } from 'react'
import { call, native } from '@/lib/api'
import { saveTabs } from '@/lib/tabs'
import {
  createRetainedViews,
  scheduleRetainedViews,
  shouldRememberLastPath,
  type RetainedViews,
  type RetainedViewServices,
} from './appSessionPersistence'

export interface AppSessionPersistenceServices extends RetainedViewServices {
  saveTabs(tabs: string[], activeTab: number): void
  rememberLastPath(path: string): void
}

export interface AppSessionPersistenceOptions {
  tabs: string[]
  activeTab: number
  path: string
  viewReferences: readonly string[]
  durablePaths: readonly string[]
  setError(message: string): void
}

const defaultServices: AppSessionPersistenceServices = {
  native,
  saveTabs,
  rememberLastPath: (path) => localStorage.setItem('fiex:lastPath', path),
  retain: (payload) => call('vfs_retain_views', payload),
  schedule: (callback, delay) => window.setTimeout(callback, delay),
  cancel: (timer) => window.clearTimeout(timer),
}

/** Persists the workspace session and retains native VFS views. */
export function useAppSessionPersistence(
  options: AppSessionPersistenceOptions,
  services: AppSessionPersistenceServices = defaultServices,
) {
  const { tabs, activeTab, path, viewReferences, durablePaths, setError } = options
  const retainedViews = JSON.stringify(createRetainedViews(viewReferences, durablePaths))

  useEffect(() => {
    if (tabs.length) services.saveTabs(tabs, activeTab)
  }, [tabs, activeTab, services])

  useEffect(() => {
    if (shouldRememberLastPath(path)) services.rememberLastPath(path)
  }, [path, services])

  useEffect(() => {
    return scheduleRetainedViews(JSON.parse(retainedViews) as RetainedViews, services, setError)
  }, [retainedViews, setError, services])
}

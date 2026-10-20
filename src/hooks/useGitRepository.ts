import { useCallback, useEffect, useRef, useState } from 'react'
import { call } from '@/lib/api'
import type { GitCommit, GitRefs } from '@/lib/types'

export function useGitRepository(
  root: string | undefined,
  path: string,
  open: boolean,
  refresh: () => Promise<void>,
  viewRevision: (reference: string) => Promise<void>,
) {
  const [commits, setCommits] = useState<GitCommit[]>([])
  const [refs, setRefs] = useState<GitRefs | null>(null)
  const [identity, setIdentity] = useState({ name: '', email: '' })
  const [message, setMessage] = useState('')
  const [busy, setBusy] = useState('')
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')
  const scope = root || path
  const latest = useRef(scope)
  latest.current = scope
  const running = useRef(false)
  useEffect(() => {
    setRefs(null)
    setCommits([])
    setIdentity({ name: '', email: '' })
    setMessage('')
    setError('')
    setSuccess('')
  }, [scope])
  const loadRefs = useCallback(async () => {
    if (!root) return
    const next = await call<GitRefs>('git_refs', { root })
    if (latest.current === root) setRefs(next)
  }, [root])
  useEffect(() => {
    if (!open || !root) return
    let active = true
    void call<GitCommit[]>('git_history', { root, skip: 0 })
      .then((items) => {
        if (active) setCommits(items)
      })
      .catch((e) => {
        if (active) setError(String(e))
      })
    void loadRefs().catch((e) => {
      if (active) setError(String(e))
    })
    void call<{ name: string; email: string }>('git_identity', { root })
      .then((next) => {
        if (active) setIdentity(next)
      })
      .catch((e) => {
        if (active) setError(String(e))
      })
    return () => {
      active = false
    }
  }, [open, root, loadRefs])
  useEffect(() => {
    if (!open || !root) return
    const timer = window.setInterval(() => {
      if (document.visibilityState === 'visible' && !running.current)
        void refresh().catch((error) => setError(String(error)))
    }, 5000)
    return () => window.clearInterval(timer)
  }, [open, root, refresh])
  const run = async (label: string, action: () => Promise<unknown>) => {
    if (running.current) return false
    running.current = true
    setBusy(label)
    setError('')
    setSuccess('')
    try {
      await action()
      if (latest.current === scope) await refresh()
      return true
    } catch (e) {
      if (latest.current === scope) setError(String(e))
      return false
    } finally {
      running.current = false
      setBusy('')
    }
  }
  return {
    refs,
    commits,
    moreHistory: () =>
      run('正在读取历史', async () => {
        const next = await call<GitCommit[]>('git_history', { root, skip: commits.length })
        if (latest.current === scope) setCommits((previous) => [...previous, ...next])
      }),
    identity,
    setIdentity,
    message,
    setMessage,
    busy,
    error,
    success,
    refresh: () =>
      run('正在刷新', async () => {
        if (!root) return
        const [items] = await Promise.all([
          call<GitCommit[]>('git_history', { root, skip: 0 }),
          loadRefs(),
        ])
        if (latest.current === scope) setCommits(items)
      }),
    initialize: () => run('正在初始化', () => call('git_init', { path })),
    stage: (paths: string[], all = false) =>
      run('正在暂存', () => call('git_stage', { root, paths, all })),
    unstage: (paths: string[], all = false) =>
      run('正在取消暂存', () => call('git_unstage', { root, paths, all })),
    browse: (reference: string) => run('正在打开历史版本', () => viewRevision(reference)),
    commit: () =>
      run('正在提交', async () => {
        const hash = await call<string>('git_commit', { root, message, ...identity })
        if (latest.current === scope) {
          setMessage('')
          setSuccess(`提交成功 · ${hash}`)
          await loadRefs()
          const items = await call<GitCommit[]>('git_history', { root, skip: 0 })
          if (latest.current === scope) setCommits(items)
        }
      }),
  }
}

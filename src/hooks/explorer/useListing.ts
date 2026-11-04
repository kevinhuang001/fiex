import { useEntryDecorations } from './useEntryDecorations'
import { cachePresentationPatch, cachePresentations, sameDecoration } from '@/lib/entryPresentation'
import { publishPresentationIcons } from '@/lib/decorationIcons'
import { t, getLocale } from '@/i18n'
import { startTransition, useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { call, native, search } from '@/lib/api'
import { openDirectoryStream } from '@/lib/listing'
import { isVirtualResource } from '@/lib/files'
import type { Entry, EntryDecoration, Location, Preferences, RemoteConnection } from '@/lib/types'
import type { Navigation } from './useNavigation'

function sameListing(previous: Entry[], next: Entry[]): boolean {
  return (
    previous.length === next.length &&
    previous.every((entry, index) => {
      const value = next[index]
      return (
        entry.path === value.path &&
        entry.name === value.name &&
        entry.isDir === value.isDir &&
        entry.isSymlink === value.isSymlink &&
        entry.size === value.size &&
        entry.modified === value.modified &&
        entry.created === value.created &&
        entry.extension === value.extension &&
        entry.mime === value.mime &&
        entry.linkTarget === value.linkTarget &&
        entry.hasCache === value.hasCache &&
        entry.pinned === value.pinned &&
        entry.uploaded === value.uploaded &&
        entry.sync === value.sync &&
        sameArray(entry.allowedOps, value.allowedOps) &&
        entry.tags.length === value.tags.length &&
        entry.tags.every(
          (tag, tagIndex) =>
            tag.name === value.tags[tagIndex].name &&
            tag.color === value.tags[tagIndex].color &&
            tag.hex === value.tags[tagIndex].hex,
        )
      )
    })
  )
}

function sameArray(previous: string[] | undefined, next: string[] | undefined): boolean {
  return (
    previous === next ||
    (previous !== undefined &&
      next !== undefined &&
      previous.length === next.length &&
      previous.every((value, index) => value === next[index]))
  )
}

/** Everything a pane lists: entries, decorations, locations, connections,
 *  search results and current-directory capabilities. Path, history and tab
 *  state come from useNavigation. */
export function useListing(
  prefs: Preferences,
  nav: Navigation,
  enabled = true,
  pane = 'left',
  startOverride?: string,
) {
  const {
    path,
    query,
    globalSearch,
    searchMode,
    history,
    navigate,
    setSelected,
    setRenaming,
    setError,
  } = nav
  const [locations, setLocations] = useState<Location[]>([])
  const [entries, setEntries] = useState<Entry[]>([])
  const [tagFilter, setTagFilterState] = useState<string | null>(null)
  const [directoryAllowedOps, setDirectoryAllowedOps] = useState<string[]>([])
  const [virtualVolume, setVirtualVolume] = useState(false)
  const [defaultView, setDefaultView] = useState<'list' | 'icons' | 'gallery' | 'columns' | null>(
    null,
  )
  const [loading, setLoading] = useState(false)
  const [searching, setSearching] = useState(false)
  const [searchStats, setSearchStats] = useState('')
  const [refresh, setRefresh] = useState(0)
  const [warnings, setWarnings] = useState<string[]>([])
  const [directoryDecorations, setDirectoryDecorations] = useState<Record<string, EntryDecoration>>(
    {},
  )
  const searchDecorations = useEntryDecorations({
    entries,
    path,
    enabled: enabled && (Boolean(query) || !native),
    query,
    refresh,
  })
  const decorations = query || !native ? searchDecorations : directoryDecorations
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [listedPath, setListedPath] = useState('')
  const request = useRef(0)
  const locationsRequest = useRef(0)
  const searchRequest = useRef(0)
  const cancelQueue = useRef<Promise<unknown>>(Promise.resolve())
  const reload = useCallback(() => setRefresh((v) => v + 1), [getLocale()])
  useEffect(() => {
    if (!native || !enabled) return
    const stop = listen('remote-sync', () => {
      if (
        !document.hidden &&
        connections.some(
          (c) => path.startsWith(c.mountPath.replace(/\/$/, '') + '/') || path === c.mountPath,
        )
      )
        reload()
    })
    return () => {
      void stop.then((unlisten) => unlisten())
    }
  }, [enabled, connections, path, reload])
  const locationsKey = useRef('')
  const locationsPending = useRef<Promise<Location[]> | null>(null)
  const refreshLocations = useCallback(
    (force = false): Promise<Location[]> => {
      if (locationsPending.current && !force) return locationsPending.current
      const request = ++locationsRequest.current
      const pending = call<Location[]>('locations')
        .then((next) => {
          if (request !== locationsRequest.current) return next
          const key = JSON.stringify(next)
          if (key !== locationsKey.current) {
            locationsKey.current = key
            setLocations(next)
          }
          return next
        })
        .finally(() => {
          if (request === locationsRequest.current) locationsPending.current = null
        })
      locationsPending.current = pending
      return pending
    },
    [getLocale()],
  )
  useEffect(() => {
    if (!enabled || pane !== 'left') return
    const check = () => {
      if (!document.hidden) void refreshLocations().catch(() => {})
    }
    const timer = window.setInterval(check, 10000)
    window.addEventListener('focus', check)
    document.addEventListener('visibilitychange', check)
    return () => {
      window.clearInterval(timer)
      window.removeEventListener('focus', check)
      document.removeEventListener('visibilitychange', check)
    }
  }, [refreshLocations, enabled, pane])
  useEffect(() => {
    if (!enabled || path) return
    let cancelled = false
    // The start folder must not wait for the disk survey: resolve it first so
    // the first listing is requested immediately, then fill the sidebar.
    void refreshLocations().catch(() => {})
    void (async () => {
      // A folder passed on the command line wins over the start page.
      const launched = native && (await call<string | null>('launch_path').catch(() => null))
      if (cancelled) return
      let start: string | null = startOverride || launched || null
      if (!start && prefs.startPage === 'recent') start = 'recent://'
      else if (!start && prefs.startPage === 'last') {
        start = await call<string | null>('state_get', {
          namespace: 'explorer',
          key: 'lastPath',
        }).catch(() => null)
        if (!start) start = localStorage.getItem('fiex:lastPath')
      }
      if (!start) {
        const initial = native ? await call<string | null>('initial_path').catch(() => null) : null
        if (cancelled) return
        const locations = await call<Location[]>('locations').catch(() => [] as Location[])
        if (cancelled) return
        const pick = (name: string) => locations.find((item) => item.name === name)?.path
        start =
          prefs.startPage === 'home'
            ? (pick('个人文件夹') ?? initial)
            : (pick('文稿') ?? initial ?? locations[0]?.path ?? null)
      }
      if (start) {
        nav.startAt(start)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [refreshLocations, enabled])
  useEffect(() => {
    // Persist only successfully opened locations, not incomplete navigation.
    if (!enabled || !path || query || listedPath !== path) return
    void call('state_put', {
      namespace: 'explorer',
      key: 'lastPath',
      value: path,
    })
      .then(() => localStorage.removeItem('fiex:lastPath'))
      .catch(() => localStorage.setItem('fiex:lastPath', path))
  }, [enabled, path, listedPath, query])

  useEffect(() => {
    if (!enabled || !path || query || path.startsWith('fiex-plugin-view://')) return
    let active = true
    const id = ++request.current
    let paintedEntries: Entry[] | null = null
    setLoading(true)
    setError('')
    void openDirectoryStream(path, prefs.hidden, prefs.displayIgnore, (snapshot) => {
      if (!active || id !== request.current) return
      const l = snapshot.listing
      if (!l) {
        if (paintedEntries === null) return
        cachePresentationPatch(paintedEntries, snapshot.decorations)
        publishPresentationIcons(paintedEntries, snapshot.presentationPending)
        if (snapshot.decorations.length) {
          setDirectoryDecorations((previous) => {
            let changed = false
            const next = { ...previous }
            for (const value of snapshot.decorations) {
              const old = previous[value.path]
              if (old && sameDecoration(old, value)) continue
              next[value.path] = value
              changed = true
            }
            return changed ? next : previous
          })
        }
        if (
          snapshot.complete &&
          path === 'recent://' &&
          paintedEntries.length === 0 &&
          history.length <= 1
        ) {
          const home = locations.find((item) => item.name === '个人文件夹')?.path
          if (home) navigate(home)
        }
        return
      }
      if (l.path !== path) {
        navigate(l.path)
        return
      }
      const displayedEntries = l.entries
      cachePresentations(l.entries, snapshot.decorations)
      publishPresentationIcons(l.entries, snapshot.presentationPending)
      setDirectoryDecorations((previous) => {
        if (
          Object.keys(previous).length === snapshot.decorations.length &&
          snapshot.decorations.every((value) => {
            const old = previous[value.path]
            return old !== undefined && sameDecoration(old, value)
          })
        )
          return previous
        return Object.fromEntries(snapshot.decorations.map((value) => [value.path, value]))
      })
      setDirectoryAllowedOps(l.allowedOps || [])
      setVirtualVolume(Boolean(l.virtualVolume))
      setDefaultView(l.defaultView || null)
      setWarnings((previous) =>
        previous.length === l.warnings.length &&
        previous.every((warning, index) => warning === l.warnings[index])
          ? previous
          : l.warnings,
      )
      if (paintedEntries === null || !sameListing(paintedEntries, displayedEntries)) {
        paintedEntries = displayedEntries
        setEntries(displayedEntries)
        const paths = new Set(l.entries.map((entry) => entry.path))
        setSelected((selected) => selected.filter((p) => paths.has(p)))
      }
      setListedPath(path)
      setLoading(false)
      if (
        snapshot.complete &&
        path === 'recent://' &&
        l.entries.length === 0 &&
        history.length <= 1
      ) {
        const home = locations.find((item) => item.name === '个人文件夹')?.path
        if (home) navigate(home)
      }
    })
      .catch((error) => {
        if (!active || id !== request.current) return
        if (paintedEntries === null) {
          setEntries([])
          setDirectoryDecorations({})
          setError(String(error))
        } else {
          // An offline refresh must not remove a correctly authorized snapshot.
          setWarnings([String(error)])
        }
      })
      .finally(() => {
        if (active && id === request.current) setLoading(false)
      })
    return () => {
      active = false
      if (paintedEntries) publishPresentationIcons(paintedEntries, false)
    }
  }, [enabled, path, prefs.hidden, prefs.displayIgnore, refresh, query])
  useEffect(() => {
    if (
      !enabled ||
      !path ||
      !native ||
      virtualVolume ||
      isVirtualResource(path) ||
      path.startsWith('fiex-plugin-view://')
    )
      return
    let cleanup: (() => void) | undefined
    let active = true
    let timer: ReturnType<typeof setTimeout>
    void call('watch_directory', { path, pane }).catch((e) =>
      setWarnings([t('无法监视文件变更：{0}', e)]),
    )
    void listen('directory-changed', () => {
      clearTimeout(timer)
      timer = setTimeout(reload, 250)
    }).then((fn) => {
      if (active) cleanup = fn
      else fn()
    })
    return () => {
      active = false
      clearTimeout(timer)
      cleanup?.()
      void call('unwatch_directory', { path, pane }).catch(() => {})
    }
  }, [enabled, path, pane, reload, virtualVolume])
  useEffect(() => {
    if (!enabled || !path) return
    const generation = ++searchRequest.current
    const cancel = () => {
      cancelQueue.current = cancelQueue.current
        .then(() => call('cancel_search', { pane }))
        .catch(() => {})
      return cancelQueue.current
    }
    const cancelled = cancel()
    if (!query.trim()) {
      setSearching(false)
      setSearchStats('')
      return
    }
    setSearching(true)
    setLoading(false)
    setEntries([])
    setSelected([])
    setError('')
    const root = globalSearch ? 'everywhere://' : path
    // A Core-owned virtual root only has an in-process entry set; local search
    // providers cannot enumerate it, so the engine stays builtin.
    const engine = 'auto'
    const timer = setTimeout(() => {
      void cancelled
        .then(() => {
          if (generation !== searchRequest.current) return
          return search(
            pane,
            root,
            query,
            prefs.hidden,
            prefs.displayIgnore,
            searchMode,
            engine,
            (b) => {
              if (generation !== searchRequest.current) return
              if (b.replace) {
                // Virtual-folder results are complete snapshots.
                startTransition(() =>
                  setEntries((previous) =>
                    sameListing(previous, b.entries) ? previous : b.entries,
                  ),
                )
              } else if (b.entries.length) {
                startTransition(() =>
                  setEntries((previous) => {
                    const merged = new Map(previous.map((entry) => [entry.path, entry]))
                    let changed = false
                    for (const entry of b.entries) {
                      if (JSON.stringify(merged.get(entry.path)) !== JSON.stringify(entry)) {
                        merged.set(entry.path, entry)
                        changed = true
                      }
                    }
                    return changed ? [...merged.values()] : previous
                  }),
                )
              }
              if (b.done) {
                setSearching(false)
                const warning = b.warning ? `${b.warning} · ` : ''
                setSearchStats(
                  `${warning}${b.elapsedMs} ms${b.truncated ? t(' · 已达到 500 项上限，请缩小范围') : ''}${b.skipped ? t(' · {0} 个位置无法访问', b.skipped) : ''}`,
                )
              }
            },
          )
        })
        .catch((e) => {
          if (generation === searchRequest.current) {
            setError(String(e))
            setSearching(false)
          }
        })
    }, 200)
    return () => {
      clearTimeout(timer)
      ++searchRequest.current
      void cancel()
    }
  }, [
    enabled,
    pane,
    query,
    globalSearch,
    path,
    searchMode,
    prefs.hidden,
    prefs.displayIgnore,
    refresh,
  ])
  useEffect(() => {
    setTagFilterState(null)
  }, [path])
  const beginRename = useCallback(
    (target: string, createdEntry?: Entry) => {
      const entry = entries.find((item) => item.path === target) ?? createdEntry
      if (entry?.path !== target || !entry.allowedOps?.includes('rename')) return
      if (!entries.some((item) => item.path === target))
        setEntries((previous) => [...previous, entry])
      setSelected([target])
      setRenaming(target)
    },
    [entries, getLocale()],
  )
  const cancelRename = useCallback(() => setRenaming(null), [getLocale()])
  const renameTo = useCallback(
    async (target: string, name: string) => {
      if (!entries.find((entry) => entry.path === target)?.allowedOps?.includes('rename')) {
        setError(t('此位置不支持该操作'))
        return false
      }
      setError('')
      try {
        const next = await call<string>('rename_entry', { path: target, name })
        setRenaming(null)
        setSelected([next])
        setRefresh((v) => v + 1)
        return true
      } catch (e) {
        setError(String(e))
        return false
      }
    },
    [entries, getLocale()],
  )
  const visibleEntries = tagFilter
    ? entries.filter((entry) => entry.tags.some((tag) => tag.name === tagFilter))
    : entries
  const setTagFilter = (tag: string | null) => {
    setTagFilterState(tag)
    setSelected([])
  }
  const refreshConnections = useCallback(async () => {
    try {
      const list = await call<RemoteConnection[]>('remote_connections')
      setConnections(Array.isArray(list) ? [...list] : [])
    } catch {
      setConnections([])
    }
  }, [getLocale()])
  useEffect(() => {
    if (!enabled) return
    void refreshConnections()
  }, [enabled, refreshConnections])
  return {
    entries: visibleEntries,
    decorations,
    connections,
    refreshConnections,
    tagFilter,
    setTagFilter,
    directoryAllowedOps,
    virtualVolume,
    defaultView,
    locations,
    refreshLocations,
    loading,
    listedPath,
    warnings,
    searching,
    searchStats,
    reload,
    beginRename,
    cancelRename,
    renameTo,
  }
}

import { startTransition, useEffect, useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { mergeSearchBatch } from '@/features/explorer/model/searchBatch'
import { createSearchLifecycle } from '@/features/explorer/model/searchLifecycle'
import type { Batch, Entry } from '@/lib/types'

interface ListingSearchNavigation {
  query: string
  globalSearch: boolean
  searchMode: 'name' | 'content'
  setSelected: (paths: string[]) => void
  setError: (error: string) => void
}

interface ListingSearchPreferences {
  hidden: boolean
  displayIgnore: string[]
}

export interface ListingSearchServices {
  cancel: (pane: string) => Promise<unknown>
  search: (
    pane: string,
    root: string,
    query: string,
    hidden: boolean,
    ignores: string[],
    mode: 'name' | 'content',
    engine: string,
    onBatch: (batch: Batch) => void,
  ) => Promise<void>
  truncatedMessage: () => string
  skippedMessage: (count: number) => string
}

interface ListingSearchOptions {
  enabled: boolean
  pane: string
  path: string
  prefs: ListingSearchPreferences
  nav: ListingSearchNavigation
  services: ListingSearchServices
  refresh: number
  setEntries: Dispatch<SetStateAction<Entry[]>>
  setLoading: Dispatch<SetStateAction<boolean>>
}

/** Owns the cancellable streamed-search lifecycle for one explorer pane. */
export function useListingSearch({
  enabled,
  pane,
  path,
  prefs,
  nav,
  services,
  refresh,
  setEntries,
  setLoading,
}: ListingSearchOptions) {
  const { query, globalSearch, searchMode, setSelected, setError } = nav
  const [searching, setSearching] = useState(false)
  const [searchStats, setSearchStats] = useState('')
  const [lifecycle] = useState(createSearchLifecycle)

  useEffect(() => {
    if (!enabled || !path) return
    const generation = lifecycle.begin()
    const cancel = () => lifecycle.cancel(() => services.cancel(pane))
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
    const timer = setTimeout(() => {
      void cancelled
        .then(() => {
          if (!lifecycle.isCurrent(generation)) return
          return services.search(
            pane,
            root,
            query,
            prefs.hidden,
            prefs.displayIgnore,
            searchMode,
            'auto',
            (batch) => {
              if (!lifecycle.isCurrent(generation)) return
              startTransition(() => setEntries((previous) => mergeSearchBatch(previous, batch)))
              if (batch.done) {
                setSearching(false)
                const warning = batch.warning ? `${batch.warning} · ` : ''
                setSearchStats(
                  `${warning}${batch.elapsedMs} ms${batch.truncated ? services.truncatedMessage() : ''}${batch.skipped ? services.skippedMessage(batch.skipped) : ''}`,
                )
              }
            },
          )
        })
        .catch((error) => {
          if (lifecycle.isCurrent(generation)) {
            setError(String(error))
            setSearching(false)
          }
        })
    }, 200)
    return () => {
      clearTimeout(timer)
      lifecycle.invalidate()
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
    lifecycle,
    services,
    setSelected,
    setError,
    setEntries,
    setLoading,
  ])

  return { searching, searchStats }
}

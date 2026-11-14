import { startTransition, useEffect, useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { call, search } from '@/lib/api'
import { t } from '@/i18n'
import { mergeSearchBatch } from '@/features/explorer/model/searchBatch'
import { createSearchLifecycle } from '@/features/explorer/model/searchLifecycle'
import type { Entry, Preferences } from '@/lib/types'
import type { Navigation } from './useNavigation'

interface ListingSearchOptions {
  enabled: boolean
  pane: string
  path: string
  prefs: Preferences
  nav: Navigation
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
    const cancel = () => lifecycle.cancel(() => call('cancel_search', { pane }))
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
          return search(
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
                  `${warning}${batch.elapsedMs} ms${batch.truncated ? t(' · 已达到 500 项上限，请缩小范围') : ''}${batch.skipped ? t(' · {0} 个位置无法访问', batch.skipped) : ''}`,
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
    setSelected,
    setError,
    setEntries,
    setLoading,
  ])

  return { searching, searchStats }
}

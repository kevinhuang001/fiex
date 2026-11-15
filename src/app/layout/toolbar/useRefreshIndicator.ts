import { useEffect, useRef, useState } from 'react'

/** Owns the short-lived visual state shown after a listing refresh. */
export function useRefreshIndicator(refreshAll: () => void) {
  const [refreshing, setRefreshing] = useState(false)
  const timer = useRef<number | null>(null)

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current)
    },
    [],
  )

  const refresh = () => {
    refreshAll()
    setRefreshing(true)
    if (timer.current !== null) window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => setRefreshing(false), 950)
  }

  return { refreshing, refresh }
}

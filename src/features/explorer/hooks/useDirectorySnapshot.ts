import { useEffect } from 'react'
import { t } from '@/i18n'
import { cachePresentationPatch, cachePresentations } from '@/lib/entryPresentation'
import { publishPresentationIcons } from '@/lib/decorationIcons'
import { openDirectoryStream } from '@/lib/listing'
import { sameStringSet, sameVisibleListing } from '@/features/explorer/model/listingDiff'
import {
  sameDirectoryDecorations,
  type PaintedListingCache,
} from '@/features/explorer/model/paintedListingCache'
import { decideDirectoryStreamUpdate } from '@/features/explorer/model/directoryStreamUpdate'
import type { Entry, EntryDecoration, Listing, Location, Preferences } from '@/lib/types'
import type { Navigation } from './useNavigation'

/** Listing stream reconciliation, including the cache-before-refresh paint. */
export interface DirectorySnapshotState {
  currentViewKey: { current: string }
  entries: { current: Entry[] }
  paintedByPath: { current: PaintedListingCache }
  renderedKey: { current: string }
  renderedPathRef: { current: string }
  request: { current: number }
  setEntries: (value: Entry[] | ((previous: Entry[]) => Entry[])) => void
  setDirectoryAllowedOps: (value: string[] | ((previous: string[]) => string[])) => void
  setVirtualVolume: (value: boolean) => void
  setDefaultView: (value: 'list' | 'icons' | 'gallery' | 'columns' | null) => void
  setWarnings: (value: string[] | ((previous: string[]) => string[])) => void
  setDirectoryDecorations: (
    value:
      | Record<string, EntryDecoration>
      | ((previous: Record<string, EntryDecoration>) => Record<string, EntryDecoration>),
  ) => void
  setListedPath: (value: string) => void
  setRenderedPath: (value: string) => void
  setLoading: (value: boolean) => void
  setError: (value: string) => void
}

export function useDirectorySnapshot({
  enabled,
  path,
  query,
  prefs,
  revalidation,
  nav,
  locations,
  state,
}: {
  enabled: boolean
  path: string
  query: string
  prefs: Pick<Preferences, 'hidden' | 'displayIgnore' | 'sort' | 'descending'>
  revalidation: number
  nav: Navigation
  locations: Location[]
  state: DirectorySnapshotState
}) {
  const { clearPendingSelection, history, navigate, pendingSelection, setSelected } = nav
  const {
    currentViewKey,
    entries,
    paintedByPath,
    renderedKey,
    renderedPathRef,
    request,
    setEntries,
    setDirectoryAllowedOps,
    setVirtualVolume,
    setDefaultView,
    setWarnings,
    setDirectoryDecorations,
    setListedPath,
    setRenderedPath,
    setLoading,
    setError,
  } = state

  useEffect(() => {
    if (!enabled || !path || query || path.startsWith('fiex-plugin-view://')) return
    let active = true
    const id = ++request.current
    const viewKey = JSON.stringify([path, prefs.hidden, prefs.displayIgnore])
    const isNewView = renderedKey.current !== viewKey
    let paintedSnapshot = paintedByPath.current.get(viewKey) ?? null
    const applyDirectoryMetadata = (listing: Listing, decorations: EntryDecoration[]) => {
      setDirectoryAllowedOps((previous) =>
        sameStringSet(previous, listing.allowedOps || []) ? previous : listing.allowedOps || [],
      )
      setVirtualVolume(Boolean(listing.virtualVolume))
      setDefaultView(listing.defaultView || null)
      setWarnings((previous) =>
        previous.length === listing.warnings.length &&
        previous.every((warning, index) => warning === listing.warnings[index])
          ? previous
          : listing.warnings,
      )
      setDirectoryDecorations((previous) => {
        const next = Object.fromEntries(decorations.map((value) => [value.path, value]))
        const oldValues = Object.values(previous)
        return sameDirectoryDecorations(oldValues, decorations) ? previous : next
      })
    }
    const navigateHome = () => {
      const home = locations.find((item) => item.name === '个人文件夹')?.path
      if (home) navigate(home)
    }
    if (paintedSnapshot) {
      paintedByPath.current.touch(viewKey)
      if (
        renderedKey.current !== viewKey ||
        !sameVisibleListing(
          entries.current,
          paintedSnapshot.listing.entries,
          prefs.sort,
          prefs.descending,
        )
      ) {
        renderedKey.current = viewKey
        renderedPathRef.current = path
        setRenderedPath(path)
        setEntries(paintedSnapshot.listing.entries)
      }
      applyDirectoryMetadata(paintedSnapshot.listing, paintedSnapshot.decorations)
      setListedPath(path)
      setLoading(false)
    } else {
      setLoading(renderedPathRef.current !== path)
    }
    if (paintedSnapshot === null || isNewView) setError('')
    void openDirectoryStream(path, prefs.hidden, prefs.displayIgnore, (snapshot) => {
      if (!active || id !== request.current) return
      const decision = decideDirectoryStreamUpdate({
        path,
        update: snapshot,
        previous: paintedSnapshot,
        sort: prefs.sort,
        descending: prefs.descending,
        historyLength: history.length,
        pendingSelection,
      })
      if (decision.kind === 'ignore') return
      if (decision.kind === 'navigate') {
        navigate(decision.path)
        return
      }
      if (decision.kind === 'patch') {
        paintedSnapshot = decision.snapshot
        paintedByPath.current.remember(viewKey, decision.snapshot)
        cachePresentationPatch(decision.snapshot.listing.entries, decision.patchDecorations)
        publishPresentationIcons(decision.snapshot.listing.entries, decision.presentationPending)
        if (decision.shouldApplyDecorations)
          setDirectoryDecorations(
            Object.fromEntries(decision.snapshot.decorations.map((value) => [value.path, value])),
          )
        if (decision.redirectHome) navigateHome()
        return
      }
      const listing = decision.snapshot.listing
      cachePresentations(listing.entries, decision.decorations)
      publishPresentationIcons(listing.entries, snapshot.presentationPending)
      paintedSnapshot = decision.snapshot
      paintedByPath.current.remember(viewKey, decision.snapshot)
      applyDirectoryMetadata(listing, decision.decorations)
      if (decision.selection.kind === 'select') {
        setSelected([decision.selection.target])
        clearPendingSelection(decision.selection.generation)
      } else if (decision.selection.kind === 'missing') {
        clearPendingSelection(decision.selection.generation)
        setError(t('无法定位已打开的文件'))
      }
      if (renderedKey.current !== viewKey || decision.visibleChanged) {
        renderedKey.current = viewKey
        renderedPathRef.current = path
        setRenderedPath(path)
        setEntries(decision.entries)
        const paths = new Set(decision.entries.map((entry) => entry.path))
        setSelected((selected) => selected.filter((item) => paths.has(item)))
      }
      setListedPath(path)
      setLoading(false)
      if (decision.redirectHome) navigateHome()
    })
      .catch((error) => {
        if (!active || id !== request.current) return
        if (paintedSnapshot === null) {
          setEntries([])
          setDirectoryDecorations({})
          setError(String(error))
        } else {
          setWarnings([String(error)])
        }
      })
      .finally(() => {
        if (active && id === request.current) setLoading(false)
      })
    return () => {
      active = false
      if (currentViewKey.current !== viewKey && paintedSnapshot)
        publishPresentationIcons(paintedSnapshot.listing.entries, false)
    }
  }, [
    enabled,
    path,
    prefs.hidden,
    prefs.displayIgnore,
    prefs.sort,
    prefs.descending,
    revalidation,
    query,
    locations,
    currentViewKey,
    entries,
    paintedByPath,
    renderedKey,
    renderedPathRef,
    request,
    setEntries,
    setDirectoryAllowedOps,
    setVirtualVolume,
    setDefaultView,
    setWarnings,
    setDirectoryDecorations,
    setListedPath,
    setRenderedPath,
    setLoading,
    setError,
    clearPendingSelection,
    history,
    navigate,
    pendingSelection,
    setSelected,
  ])
  currentViewKey.current = JSON.stringify([path, prefs.hidden, prefs.displayIgnore])
}

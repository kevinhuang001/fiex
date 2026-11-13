import { useEffect } from 'react'
import { t } from '@/i18n'
import { cachePresentationPatch, cachePresentations } from '@/lib/entryPresentation'
import { publishPresentationIcons } from '@/lib/decorationIcons'
import { openDirectoryStream } from '@/lib/listing'
import { sameStringSet, sameVisibleListing } from '@/lib/listingDiff'
import {
  sameDirectoryDecorations,
  sameListingMetadata,
  type PaintedListingCache,
} from '@/features/explorer/model/paintedListingCache'
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
      if (paintedSnapshot && snapshot.revision < paintedSnapshot.revision) return
      const listing = snapshot.listing
      if (!listing) {
        if (!paintedSnapshot) return
        const decorations = new Map(paintedSnapshot.decorations.map((value) => [value.path, value]))
        for (const value of snapshot.decorations) decorations.set(value.path, value)
        const nextDecorations = [...decorations.values()]
        paintedSnapshot = {
          ...paintedSnapshot,
          decorations: nextDecorations,
          revision: snapshot.revision,
        }
        paintedByPath.current.remember(viewKey, paintedSnapshot)
        cachePresentationPatch(paintedSnapshot.listing.entries, snapshot.decorations)
        publishPresentationIcons(paintedSnapshot.listing.entries, snapshot.presentationPending)
        if (snapshot.decorations.length)
          setDirectoryDecorations(
            Object.fromEntries(nextDecorations.map((value) => [value.path, value])),
          )
        if (
          snapshot.complete &&
          path === 'recent://' &&
          paintedSnapshot.listing.entries.length === 0 &&
          nav.history.length <= 1
        ) {
          const home = locations.find((item) => item.name === '个人文件夹')?.path
          if (home) nav.navigate(home)
        }
        return
      }
      if (listing.path !== path) {
        nav.navigate(listing.path)
        return
      }
      const pending = nav.pendingSelection
      if (pending?.directory === path) {
        if (listing.entries.some((entry) => entry.path === pending.target)) {
          nav.setSelected([pending.target])
          nav.clearPendingSelection(pending.generation)
        } else if (snapshot.complete) {
          nav.clearPendingSelection(pending.generation)
          setError(t('无法定位已打开的文件'))
        }
      }
      cachePresentations(listing.entries, snapshot.decorations)
      publishPresentationIcons(listing.entries, snapshot.presentationPending)
      const previousPainted = paintedSnapshot?.listing.entries ?? null
      const visibleUnchanged =
        paintedSnapshot !== null &&
        sameVisibleListing(
          paintedSnapshot.listing.entries,
          listing.entries,
          prefs.sort,
          prefs.descending,
        )
      const stableEntries =
        paintedSnapshot && visibleUnchanged ? paintedSnapshot.listing.entries : listing.entries
      const nextListing = { ...listing, entries: stableEntries }
      const stableListing =
        paintedSnapshot &&
        visibleUnchanged &&
        sameListingMetadata(paintedSnapshot.listing, nextListing)
          ? paintedSnapshot.listing
          : nextListing
      paintedSnapshot = {
        listing: stableListing,
        decorations: snapshot.decorations,
        revision: snapshot.revision,
      }
      paintedByPath.current.remember(viewKey, paintedSnapshot)
      applyDirectoryMetadata(stableListing, snapshot.decorations)
      if (
        renderedKey.current !== viewKey ||
        !previousPainted ||
        !sameVisibleListing(previousPainted, stableEntries, prefs.sort, prefs.descending)
      ) {
        renderedKey.current = viewKey
        renderedPathRef.current = path
        setRenderedPath(path)
        setEntries(stableEntries)
        const paths = new Set(listing.entries.map((entry) => entry.path))
        nav.setSelected((selected) => selected.filter((item) => paths.has(item)))
      }
      setListedPath(path)
      setLoading(false)
      if (
        snapshot.complete &&
        path === 'recent://' &&
        listing.entries.length === 0 &&
        nav.history.length <= 1
      ) {
        const home = locations.find((item) => item.name === '个人文件夹')?.path
        if (home) nav.navigate(home)
      }
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
    nav.pendingSelection,
    nav.clearPendingSelection,
  ])
  currentViewKey.current = JSON.stringify([path, prefs.hidden, prefs.displayIgnore])
}

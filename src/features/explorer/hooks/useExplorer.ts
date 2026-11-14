import type { Preferences } from '@/lib/types'
import { call } from '@/lib/api'
import { parent } from '@/lib/files'
import { useListing } from './useListing'
import { useNavigation, type NavigationServices } from './useNavigation'
import { explorerListingServices } from '@/features/explorer/services/listingAdapters'

const navigationServices: NavigationServices = {
  readSearchMode: () => localStorage.getItem('fiex:search-mode'),
  writeSearchMode: (mode) => localStorage.setItem('fiex:search-mode', mode),
  parentPath: parent,
  resolveParentPath: (path, fallback) =>
    path.startsWith('applications://') ? call('file_parent', { path }) : Promise.resolve(fallback),
}

export function useExplorer(
  prefs: Preferences,
  startOverride?: string,
  enabled = true,
  pane = 'left',
  initialTabs?: { paths: string[]; active: number },
) {
  const nav = useNavigation(navigationServices, initialTabs)
  const listing = useListing(prefs, nav, explorerListingServices, enabled, pane, startOverride)
  return {
    path: nav.path,
    previousPath: nav.previousPath,
    referencedPaths: nav.referencedPaths,
    entries: listing.entries,
    decorations: listing.decorations,
    connections: listing.connections,
    connectionsLoaded: listing.connectionsLoaded,
    refreshConnections: listing.refreshConnections,
    tagFilter: listing.tagFilter,
    setTagFilter: listing.setTagFilter,
    directoryAllowedOps: listing.directoryAllowedOps,
    virtualVolume: listing.virtualVolume,
    defaultView: listing.defaultView,
    locations: listing.locations,
    refreshLocations: listing.refreshLocations,
    selected: nav.selected,
    setSelected: nav.setSelected,
    loading: listing.loading || (!nav.query && listing.listedPath !== nav.path && !nav.error),
    hasListing: listing.listedPath === nav.path,
    error: nav.error,
    setError: nav.setError,
    warnings: listing.warnings,
    query: nav.query,
    setQuery: nav.setQuery,
    globalSearch: nav.globalSearch,
    setGlobalSearch: nav.setGlobalSearch,
    searchMode: nav.searchMode,
    setSearchMode: nav.setSearchMode,
    searching: listing.searching,
    searchStats: listing.searchStats,
    navigate: nav.navigate,
    navigateTo: nav.navigateTo,
    reload: listing.reload,
    back: nav.back,
    forward: nav.forward,
    canBack: nav.canBack,
    canForward: nav.canForward,
    up: nav.up,
    tabs: nav.tabs,
    tab: nav.tab,
    newTab: nav.newTab,
    switchTab: nav.switchTab,
    closeTab: nav.closeTab,
    moveTab: nav.moveTab,
    snapshotNavigation: nav.snapshotNavigation,
    restoreNavigation: nav.restoreNavigation,
    renaming: nav.renaming,
    beginRename: listing.beginRename,
    cancelRename: listing.cancelRename,
    renameTo: listing.renameTo,
  }
}
export type Explorer = ReturnType<typeof useExplorer>

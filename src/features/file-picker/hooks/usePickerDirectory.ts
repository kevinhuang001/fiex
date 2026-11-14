import { useEffect, useRef, useState, type Dispatch, type SetStateAction } from 'react'
import type { FilePickerRequest } from '@/lib/filePicker'
import type { Entry, Location, RemoteConnection } from '@/lib/types'
import {
  persistPickerDirectory,
  persistPickerSort,
  storedDirectory,
  storedGroup,
  storedSort,
} from '../services/preferences'
import type { GroupKey, PickerDirectoryServices, PickerSort } from '../types'

function usableConnections(items: RemoteConnection[], mode: FilePickerRequest['mode']) {
  return items.filter(
    (item) =>
      item.mountPath.startsWith('fiex://mount/') &&
      (mode === 'saveFile' ? item.canWrite === true : item.canRead !== false),
  )
}

function isPickerSort(value: unknown): value is PickerSort {
  if (!value || typeof value !== 'object') return false
  const sort = value as Record<string, unknown>
  return ['name', 'modified', 'size', 'kind'].includes(String(sort.key))
}

function isGroupKey(value: unknown): value is GroupKey {
  return typeof value === 'string' && ['none', 'kind', 'modified', 'size'].includes(value)
}

/** Owns picker directory selection, remembered preferences, and directory listing lifecycle. */
export function usePickerDirectory(
  request: FilePickerRequest,
  setError: Dispatch<SetStateAction<string>>,
  services: PickerDirectoryServices,
) {
  const initialSidebar = services.sidebarSnapshot()
  const rememberedDirectory = useRef(storedDirectory(services.preferences.storage))
  const navigationChanged = useRef(false)
  const initialDirectory =
    request.directory ||
    rememberedDirectory.current ||
    (initialSidebar?.locations.find((item) => item.kind === 'home') || initialSidebar?.locations[0])
      ?.path ||
    ''
  const initialDirectoryRef = useRef(initialDirectory)
  const [directory, setDirectory] = useState(initialDirectory)
  const [entries, setEntries] = useState<Entry[]>([])
  const [loading, setLoading] = useState(true)
  const [locations, setLocations] = useState<Location[]>(initialSidebar?.locations || [])
  const [connections, setConnections] = useState<RemoteConnection[]>(() =>
    usableConnections(initialSidebar?.connections || [], request.mode),
  )
  const [history, setHistory] = useState<string[]>(initialDirectory ? [initialDirectory] : [])
  const [index, setIndex] = useState(initialDirectory ? 0 : -1)
  const [sort, setSort] = useState(() => storedSort(services.preferences.storage))
  const [settingsReady, setSettingsReady] = useState(false)
  const [group, setGroup] = useState<GroupKey>(() => storedGroup(services.preferences.storage))
  const [reload, setReload] = useState(0)
  const locationsRef = useRef(locations)

  useEffect(() => {
    locationsRef.current = locations
  }, [locations])

  const navigate = (path: string) => {
    if (!path || path === directory) return false
    navigationChanged.current = true
    setDirectory(path)
    setEntries([])
    setLoading(true)
    setHistory((old) => [...old.slice(0, index + 1), path])
    setIndex((old) => old + 1)
    return true
  }

  useEffect(() => {
    let alive = true
    void services
      .readSettings(request.mode)
      .then(({ sort: savedSort, group: savedGroup, directory: savedDirectory }) => {
        if (!alive) return
        if (isPickerSort(savedSort)) {
          setSort({
            key: savedSort.key,
            direction: savedSort.direction === 'desc' ? 'desc' : 'asc',
          })
        }
        if (isGroupKey(savedGroup)) setGroup(savedGroup)
        if (
          !request.directory &&
          typeof savedDirectory === 'string' &&
          savedDirectory &&
          !navigationChanged.current
        ) {
          rememberedDirectory.current = savedDirectory
          setDirectory(savedDirectory)
          setHistory([savedDirectory])
          setIndex(0)
        }
        setSettingsReady(true)
      })
      .catch(() => {
        if (alive) setSettingsReady(true)
      })
    return () => {
      alive = false
    }
  }, [request.directory, request.mode, services])

  useEffect(() => {
    let alive = true
    void services
      .loadSidebarData()
      .then(({ locations, connections }) => {
        if (!alive) return
        setLocations(locations)
        setConnections(usableConnections(connections, request.mode))
        if (!initialDirectoryRef.current) {
          const target =
            request.directory ||
            rememberedDirectory.current ||
            (locations.find((item) => item.kind === 'home') || locations[0])?.path ||
            ''
          if (target) {
            initialDirectoryRef.current = target
            setDirectory(target)
            setHistory([target])
            setIndex(0)
          }
        }
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [request.directory, request.mode, services])

  useEffect(() => {
    let alive = true
    if (!directory) {
      setEntries([])
      setError('')
      setLoading(false)
      return () => {
        alive = false
      }
    }
    setError('')
    setLoading(true)
    void services
      .openDirectory(directory, false, [], (value) => {
        if (!alive) return
        setEntries(value.entries || [])
        setLoading(false)
        void persistPickerDirectory(directory, request.mode, services.preferences)
        if (rememberedDirectory.current === directory) rememberedDirectory.current = ''
      })
      .catch((cause) => {
        if (!alive) return
        setLoading(false)
        const currentLocations = locationsRef.current
        const home = (currentLocations.find((item) => item.kind === 'home') || currentLocations[0])
          ?.path
        if (
          !request.directory &&
          rememberedDirectory.current === directory &&
          home &&
          home !== directory
        ) {
          rememberedDirectory.current = ''
          setDirectory(home)
          setHistory([home])
          setIndex(0)
          return
        }
        setError(String(cause))
        services.reportError(cause, 'file-picker')
      })
    return () => {
      alive = false
    }
  }, [directory, reload, request.directory, request.mode, setError, services])

  useEffect(() => {
    if (settingsReady) void persistPickerSort(sort, group, services.preferences)
  }, [sort, group, settingsReady, services])

  return {
    directory,
    setDirectory,
    entries,
    loading,
    locations,
    connections,
    history,
    index,
    setIndex,
    sort,
    setSort,
    group,
    setGroup,
    setReload,
    navigate,
  }
}

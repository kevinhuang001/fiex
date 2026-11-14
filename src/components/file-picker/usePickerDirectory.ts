import { useEffect, useRef, useState, type Dispatch, type SetStateAction } from 'react'
import { call } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { openPresentedDirectory } from '@/lib/listing'
import { loadPickerSidebarData, pickerSidebarSnapshot } from '@/lib/filePickerData'
import type { FilePickerRequest } from '@/lib/filePicker'
import type { Entry, Location, RemoteConnection } from '@/lib/types'
import {
  persistPickerDirectory,
  persistPickerSort,
  storedDirectory,
  storedGroup,
  storedSort,
  type GroupKey,
  type SortKey,
} from './types'

function usableConnections(items: RemoteConnection[], mode: FilePickerRequest['mode']) {
  return items.filter(
    (item) =>
      item.mountPath.startsWith('fiex://mount/') &&
      (mode === 'saveFile' ? item.canWrite === true : item.canRead !== false),
  )
}

/** Owns picker directory selection, remembered preferences, and directory listing lifecycle. */
export function usePickerDirectory(
  request: FilePickerRequest,
  setError: Dispatch<SetStateAction<string>>,
) {
  const initialSidebar = pickerSidebarSnapshot()
  const rememberedDirectory = useRef(storedDirectory(request.mode))
  const navigationChanged = useRef(false)
  const initialDirectory =
    request.directory ||
    rememberedDirectory.current ||
    (initialSidebar?.locations.find((item) => item.kind === 'home') || initialSidebar?.locations[0])
      ?.path ||
    ''
  const [directory, setDirectory] = useState(initialDirectory)
  const [entries, setEntries] = useState<Entry[]>([])
  const [loading, setLoading] = useState(true)
  const [locations, setLocations] = useState<Location[]>(initialSidebar?.locations || [])
  const [connections, setConnections] = useState<RemoteConnection[]>(() =>
    usableConnections(initialSidebar?.connections || [], request.mode),
  )
  const [history, setHistory] = useState<string[]>(initialDirectory ? [initialDirectory] : [])
  const [index, setIndex] = useState(initialDirectory ? 0 : -1)
  const [sort, setSort] = useState(storedSort)
  const [settingsReady, setSettingsReady] = useState(false)
  const [group, setGroup] = useState<GroupKey>(storedGroup)
  const [reload, setReload] = useState(0)

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
    const lastKey = request.mode === 'saveFile' ? 'lastSaveDirectory' : 'lastOpenDirectory'
    void Promise.all([
      call('state_get', {
        namespace: 'picker',
        key: 'sort',
      }),
      call('state_get', { namespace: 'picker', key: 'group' }),
      call('state_get', { namespace: 'picker', key: lastKey }),
    ])
      .then(([savedSort, savedGroup, savedDirectory]) => {
        if (!alive) return
        if (savedSort && ['name', 'modified', 'size', 'kind'].includes(savedSort.key)) {
          setSort({
            key: savedSort.key as SortKey,
            direction: savedSort.direction === 'desc' ? 'desc' : 'asc',
          })
        }
        if (savedGroup && ['none', 'kind', 'modified', 'size'].includes(savedGroup))
          setGroup(savedGroup as GroupKey)
        if (!request.directory && savedDirectory && !navigationChanged.current) {
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
  }, [])

  useEffect(() => {
    let alive = true
    void loadPickerSidebarData()
      .then(({ locations, connections }) => {
        if (!alive) return
        setLocations(locations)
        setConnections(usableConnections(connections, request.mode))
        if (!directory) {
          const target =
            request.directory ||
            rememberedDirectory.current ||
            (locations.find((item) => item.kind === 'home') || locations[0])?.path ||
            ''
          if (target) {
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
  }, [])

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
    void openPresentedDirectory(directory, false, [], (value) => {
      if (!alive) return
      setEntries(value.entries || [])
      setLoading(false)
      persistPickerDirectory(directory, request.mode)
      if (rememberedDirectory.current === directory) rememberedDirectory.current = ''
    }).catch((cause) => {
      if (!alive) return
      setLoading(false)
      const home = (locations.find((item) => item.kind === 'home') || locations[0])?.path
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
      reportError(cause, 'file-picker')
    })
    return () => {
      alive = false
    }
  }, [directory, reload])

  useEffect(() => {
    if (settingsReady) persistPickerSort(sort, group)
  }, [sort, group, settingsReady])

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

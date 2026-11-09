import { call } from '@/lib/api'
import { detectPlatform } from '@/lib/platform'
import type { Ide, Terminal } from '@/lib/types'
import { useEffect, useState } from 'react'

export function useDesktopEnvironment() {
  const [terminals, setTerminals] = useState<Terminal[]>([])
  const [ides, setIdes] = useState<Ide[]>([])
  const [platform, setPlatform] = useState(detectPlatform)
  const [trashPath, setTrashPath] = useState<string | null>(null)
  useEffect(() => {
    void call('platform')
      .then(setPlatform)
      .catch(() => {})
    void call('detect_terminals')
      .then((list) => setTerminals(Array.isArray(list) ? list : []))
      .catch(() => {})
    void call('detect_ides')
      .then((list) => setIdes(Array.isArray(list) ? list : []))
      .catch(() => {})
    void call('trash_path')
      .then(setTrashPath)
      .catch(() => {})
  }, [])
  return { terminals, ides, platform, trashPath }
}

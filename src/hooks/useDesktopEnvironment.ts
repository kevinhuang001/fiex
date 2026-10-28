import { call } from '@/lib/api'
import { detectPlatform } from '@/lib/platform'
import type { Platform } from '@/lib/platform'
import type { Ide, Terminal } from '@/lib/types'
import { useEffect, useState } from 'react'

export function useDesktopEnvironment() {
  const [terminals, setTerminals] = useState<Terminal[]>([])
  const [ides, setIdes] = useState<Ide[]>([])
  const [platform, setPlatform] = useState(detectPlatform)
  const [trashPath, setTrashPath] = useState<string | null>(null)
  useEffect(() => {
    void call<Platform>('platform')
      .then(setPlatform)
      .catch(() => {})
    void call<Terminal[]>('detect_terminals')
      .then((list) => setTerminals(Array.isArray(list) ? list : []))
      .catch(() => {})
    void call<Ide[]>('detect_ides')
      .then((list) => setIdes(Array.isArray(list) ? list : []))
      .catch(() => {})
    void call<string | null>('trash_path')
      .then(setTrashPath)
      .catch(() => {})
  }, [])
  return { terminals, ides, platform, trashPath }
}

import { call } from '@/lib/api'
import type { Ide, Terminal } from '@/lib/types'
import { useEffect, useState } from 'react'

/** Match the native platform before the async command answers, so the window
 *  chrome (traffic lights vs. caption buttons) does not flash on first paint. */
function detectPlatform() {
  const info = navigator as Navigator & { userAgentData?: { platform?: string } }
  const name = info.userAgentData?.platform || info.platform || ''
  if (/mac|iphone|ipad/i.test(name)) return 'macos'
  if (/win/i.test(name)) return 'windows'
  if (/linux/i.test(name)) return 'linux'
  return ''
}

export function useDesktopEnvironment() {
  const [terminals, setTerminals] = useState<Terminal[]>([])
  const [ides, setIdes] = useState<Ide[]>([])
  const [platform, setPlatform] = useState(detectPlatform)
  const [trashPath, setTrashPath] = useState<string | null>(null)
  useEffect(() => {
    void call<string>('platform')
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

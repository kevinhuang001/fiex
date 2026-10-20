import { useEffect, useState } from 'react'
import type { Preferences } from '@/lib/types'
const defaults: Preferences = {
  view: 'list',
  group: 'none',
  sort: 'name',
  descending: false,
  hidden: false,
  respectIgnore: false,
  terminal: 'default',
  theme: 'system',
  iconSize: 88,
  favorites: [],
}
export function usePreferences() {
  const [prefs, setPrefs] = useState<Preferences>(() => {
    try {
      return { ...defaults, ...JSON.parse(localStorage.getItem('fiex:preferences') || '{}') }
    } catch {
      return defaults
    }
  })
  useEffect(() => {
    localStorage.setItem('fiex:preferences', JSON.stringify(prefs))
    const apply = () =>
      document.documentElement.classList.toggle(
        'dark',
        prefs.theme === 'dark' ||
          (prefs.theme === 'system' && matchMedia('(prefers-color-scheme: dark)').matches),
      )
    apply()
    const media = matchMedia('(prefers-color-scheme: dark)')
    media.addEventListener('change', apply)
    return () => media.removeEventListener('change', apply)
  }, [prefs])
  return [prefs, (p: Partial<Preferences>) => setPrefs((v) => ({ ...v, ...p }))] as const
}

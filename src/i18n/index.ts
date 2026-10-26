// Subscribe once at the App root; locale changes rerender the existing tree without remounting panels.
import { useSyncExternalStore } from 'react'
import { createI18n, normalizeLocale, type Locale } from '../../sdk/ui/src/i18n'
import en from './messages/en.json'
import traditional from './messages/zh-TW.json'
export { normalizeLocale, type Locale }
function initialLocale(): Locale {
  try {
    return normalizeLocale(JSON.parse(localStorage.getItem('fiex:preferences') || '{}').locale)
  } catch {
    return 'zh-CN'
  }
}
const runtime = createI18n({ en, 'zh-TW': traditional }, initialLocale())
export const { t, translateMessage, getLocale, subscribe: subscribeLocale } = runtime
export function setLocale(value: unknown) {
  runtime.setLocale(value)
  if (typeof document !== 'undefined') document.documentElement.lang = getLocale()
}
export const useLocale = () => useSyncExternalStore(subscribeLocale, getLocale, getLocale)

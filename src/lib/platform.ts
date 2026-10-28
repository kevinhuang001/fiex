/** Best-effort platform detection for label choices before Core answers. */
export type Platform = 'macos' | 'windows' | 'linux' | ''

export function detectPlatform(): Platform {
  if (typeof navigator === 'undefined') return ''
  const info = navigator as Navigator & { userAgentData?: { platform?: string } }
  const name = info.userAgentData?.platform || info.platform || ''
  if (/mac|iphone|ipad/i.test(name)) return 'macos'
  if (/win/i.test(name)) return 'windows'
  if (/linux/i.test(name)) return 'linux'
  return ''
}

export const isWindows = () => detectPlatform() === 'windows'

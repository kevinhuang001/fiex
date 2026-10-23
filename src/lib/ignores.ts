import { inferShortcutStyle } from './shortcuts'

const MAC = ['.fiex', '.DS_Store', '._*', '.Spotlight-V100', '.fseventsd', '.Trashes']
const WINDOWS = ['.fiex', 'Thumbs.db', 'desktop.ini', '$RECYCLE.BIN', 'System Volume Information']
const LINUX = ['.fiex', '.directory', '.Trash-*']

export function defaultDisplayIgnore(): string[] {
  const style = inferShortcutStyle()
  if (style === 'mac') return [...MAC, ...WINDOWS, ...LINUX]
  if (style === 'win') return [...WINDOWS, ...MAC, ...LINUX]
  return [...LINUX, ...MAC, ...WINDOWS]
}

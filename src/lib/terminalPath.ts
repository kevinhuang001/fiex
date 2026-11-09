import { isVirtualResource } from './files'

/** Local terminals use the current folder or fall back to the OS home folder. */
export function terminalWorkingDirectory(path: string) {
  return isVirtualResource(path) ? '' : path
}

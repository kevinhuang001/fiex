export type FilePickerMode = 'openFile' | 'openFiles' | 'openDirectory' | 'saveFile'

export interface FilePickerRequest {
  mode: FilePickerMode
  directory?: string
  suggestedName?: string
  extensions?: string[]
}

interface Pending {
  request: FilePickerRequest
  resolve: (value: string | string[] | null) => void
}

let pending: Pending | null = null
const listeners = new Set<() => void>()

export function currentFilePicker(): Pending | null {
  return pending
}

export function subscribeFilePicker(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Opens the in-app chooser. The host UI resolves it with the picked path(s)
 * or null when cancelled. */
export function openFilePicker(request: FilePickerRequest): Promise<string | string[] | null> {
  pending?.resolve(null)
  return new Promise((resolve) => {
    pending = { request, resolve }
    listeners.forEach((listener) => listener())
  })
}

export function settleFilePicker(value: string | string[] | null) {
  const current = pending
  pending = null
  listeners.forEach((listener) => listener())
  current?.resolve(value)
}
